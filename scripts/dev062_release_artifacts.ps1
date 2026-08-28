<#
.SYNOPSIS
    Stage the three AI Studio 0.8.0 Windows release artifacts and write SHA-256 checksums.

.DESCRIPTION
    This script only reads already-built artifacts. It never runs a build, changes product
    source, or writes release files inside the repository. Portable EXE version metadata and
    exact 0.8.0 installer filenames are required; missing or ambiguous artifacts fail loudly.

.EXAMPLE
    pwsh -NoProfile -File .\scripts\dev062_release_artifacts.ps1

.EXAMPLE
    pwsh -NoProfile -File .\scripts\dev062_release_artifacts.ps1 -Verify -StagingDirectory C:\Temp\AI-Studio-0.8.0-release-...
#>
[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$StagingDirectory,
    [switch]$Verify
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$ReleaseVersion = '0.8.0'
$ManifestName = "RELEASE_SHA256_$ReleaseVersion.txt"
$PortableName = 'ai-studio.exe'
$NsisName = "AI Studio_$ReleaseVersion`_x64-setup.exe"
$MsiName = "AI Studio_$ReleaseVersion`_x64_en-US.msi"

function Fail([string]$Message) {
    throw "[DEV-062] $Message"
}

function Resolve-Directory([string]$Path, [string]$Description) {
    if ([string]::IsNullOrWhiteSpace($Path)) {
        Fail "$Description path is required."
    }

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $resolved) {
        Fail "$Description does not exist: $Path"
    }

    $item = Get-Item -LiteralPath $resolved.Path -ErrorAction Stop
    if (-not $item.PSIsContainer) {
        Fail "$Description is not a directory: $($item.FullName)"
    }

    return $item.FullName
}

function Test-PathWithin([string]$Path, [string]$Root) {
    $fullPath = [IO.Path]::GetFullPath($Path).TrimEnd('\') + '\'
    $fullRoot = [IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    return $fullPath.StartsWith($fullRoot, [StringComparison]::OrdinalIgnoreCase)
}

function Get-UniqueArtifact([string]$Directory, [string]$FileName, [string]$Description) {
    if (-not (Test-Path -LiteralPath $Directory -PathType Container)) {
        Fail "$Description directory is missing: $Directory"
    }

    $candidates = @(Get-ChildItem -LiteralPath $Directory -File -Filter $FileName)
    if ($candidates.Count -eq 0) {
        Fail "Missing $Description for $ReleaseVersion. Expected exact filename '$FileName' in '$Directory'. Older-version artifacts are not acceptable."
    }
    if ($candidates.Count -ne 1) {
        $paths = ($candidates | ForEach-Object { $_.FullName }) -join '; '
        Fail "Ambiguous $Description for $ReleaseVersion. Found $($candidates.Count) exact matches: $paths"
    }

    return $candidates[0]
}

function Assert-VersionMetadata([System.IO.FileInfo]$File, [string]$Description, [bool]$RequireMetadata) {
    $values = @(
        $File.VersionInfo.ProductVersion
        $File.VersionInfo.FileVersion
    ) | Where-Object {
        -not [string]::IsNullOrWhiteSpace([string]$_)
    } | ForEach-Object {
        ([string]$_).Trim()
    } | Select-Object -Unique
    $values = @($values)

    if ($RequireMetadata -and $values.Count -eq 0) {
        Fail "$Description has no readable embedded version metadata; refusing to infer that it is $ReleaseVersion."
    }

    foreach ($value in $values) {
        if ($value -notmatch "^$([regex]::Escape($ReleaseVersion))(\.0)?$") {
            Fail "$Description has embedded version '$value', expected $ReleaseVersion. This prevents accidentally packaging an older artifact."
        }
    }
}

function Assert-VersionValue([string]$Value, [string]$Description) {
    if ([string]::IsNullOrWhiteSpace($Value)) {
        Fail "$Description has no readable version value; refusing to infer that it is $ReleaseVersion."
    }
    if ($Value.Trim() -notmatch "^$([regex]::Escape($ReleaseVersion))(\.0)?$") {
        Fail "$Description has version '$($Value.Trim())', expected $ReleaseVersion. This prevents accidentally packaging an older artifact."
    }
}

function Get-MsiProductVersion([System.IO.FileInfo]$File) {
    $installer = $null
    $database = $null
    $view = $null
    $record = $null

    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $database = $installer.OpenDatabase($File.FullName, 0)
        $view = $database.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductVersion'")
        $view.Execute()
        $record = $view.Fetch()
        if ($null -eq $record) {
            Fail "MSI has no ProductVersion property: $($File.FullName)"
        }
        return ([string]$record.StringData(1)).Trim()
    } catch {
        if ($_.Exception.Message.StartsWith('[DEV-062]')) {
            throw
        }
        Fail "Unable to inspect MSI ProductVersion for '$($File.FullName)': $($_.Exception.Message)"
    } finally {
        foreach ($com in @($record, $view, $database, $installer)) {
            if ($null -ne $com) {
                [void][Runtime.InteropServices.Marshal]::ReleaseComObject($com)
            }
        }
    }
}

function Get-SourceMetadata([string]$Root) {
    $head = 'UNKNOWN'
    $worktree = 'NOT_AVAILABLE'

    try {
        $headOutput = @(& git -C $Root rev-parse --verify HEAD 2>$null)
        if ($LASTEXITCODE -eq 0 -and $headOutput.Count -eq 1 -and $headOutput[0] -match '^[0-9a-fA-F]{40}$') {
            $head = ([string]$headOutput[0]).Trim()
            $statusOutput = @(& git -C $Root status --porcelain 2>$null)
            if ($LASTEXITCODE -eq 0) {
                if ($statusOutput.Count -eq 0) {
                    $worktree = 'CLEAN'
                } else {
                    $worktree = 'DIRTY'
                }
            }
        }
    } catch {
        $head = 'UNKNOWN'
        $worktree = 'NOT_AVAILABLE'
    }

    return [pscustomobject]@{
        Head      = $head
        Worktree  = $worktree
    }
}

function Get-ManifestEntries([string]$ManifestPath) {
    $entries = @()
    foreach ($line in @(Get-Content -LiteralPath $ManifestPath)) {
        if ($line -match '^(?<Name>[^|]+?)\s*\|\s*bytes=(?<Bytes>\d+)\s*\|\s*SHA256=(?<Hash>[A-Fa-f0-9]{64})$') {
            $entries += [pscustomobject]@{
                Name = $Matches['Name'].Trim()
                Bytes = [int64]$Matches['Bytes']
                Hash = $Matches['Hash'].ToUpperInvariant()
            }
        }
    }
    return $entries
}

function Verify-Staging([string]$Directory) {
    $manifestPath = Join-Path $Directory $ManifestName
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        Fail "Missing checksum manifest: $manifestPath"
    }

    $expectedNames = @($MsiName, $NsisName, $PortableName)
    $entries = @(Get-ManifestEntries $manifestPath)
    if ($entries.Count -ne $expectedNames.Count) {
        Fail "Checksum manifest must contain exactly three parseable artifact entries; found $($entries.Count)."
    }

    $duplicateNames = @($entries | Group-Object Name | Where-Object { $_.Count -gt 1 })
    if ($duplicateNames.Count -gt 0) {
        Fail "Checksum manifest contains duplicate artifact entries: $(($duplicateNames | ForEach-Object { $_.Name }) -join ', ')"
    }

    $unexpectedNames = @($entries | Where-Object { $expectedNames -notcontains $_.Name })
    if ($unexpectedNames.Count -gt 0) {
        Fail "Checksum manifest contains unexpected artifact entries: $(($unexpectedNames | ForEach-Object { $_.Name }) -join ', ')"
    }

    foreach ($name in $expectedNames) {
        $entry = @($entries | Where-Object { $_.Name -eq $name })
        if ($entry.Count -ne 1) {
            Fail "Checksum manifest is missing exactly one entry for '$name'."
        }

        $artifactPath = Join-Path $Directory $name
        if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
            Fail "Staged artifact referenced by manifest is missing: $artifactPath"
        }

        $file = Get-Item -LiteralPath $artifactPath
        $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifactPath).Hash.ToUpperInvariant()
        if ([int64]$file.Length -ne [int64]$entry[0].Bytes) {
            Fail "Byte count mismatch for '$name': manifest=$($entry[0].Bytes), actual=$($file.Length)."
        }
        if ($actualHash -ne $entry[0].Hash) {
            Fail "SHA-256 mismatch for '$name': manifest=$($entry[0].Hash), actual=$actualHash."
        }

        Write-Output "VERIFIED $name | bytes=$($file.Length) | SHA256=$actualHash"
    }
}

if ($Verify) {
    if ([string]::IsNullOrWhiteSpace($StagingDirectory)) {
        Fail '-Verify requires -StagingDirectory pointing to an existing release staging directory.'
    }
    $StagingDirectory = Resolve-Directory $StagingDirectory 'Staging directory'
    Verify-Staging $StagingDirectory
    Write-Output "Verification passed: $(Join-Path $StagingDirectory $ManifestName)"
    exit 0
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot '..'
}
$RepositoryRoot = Resolve-Directory $RepositoryRoot 'Repository root'

$releaseRoot = Resolve-Directory (Join-Path $RepositoryRoot 'src-tauri\target\release') 'Release artifact root'
$nsisRoot = Resolve-Directory (Join-Path $releaseRoot 'bundle\nsis') 'NSIS bundle directory'
$msiRoot = Resolve-Directory (Join-Path $releaseRoot 'bundle\msi') 'MSI bundle directory'

$portable = Get-UniqueArtifact $releaseRoot $PortableName 'portable executable'
Assert-VersionMetadata $portable 'Portable executable' $true

$nsis = Get-UniqueArtifact $nsisRoot $NsisName 'NSIS installer'
$msi = Get-UniqueArtifact $msiRoot $MsiName 'MSI installer'

Assert-VersionMetadata $nsis 'NSIS installer' $false
Assert-VersionValue (Get-MsiProductVersion $msi) 'MSI ProductVersion'

foreach ($artifact in @($portable, $nsis, $msi)) {
    if ($artifact.Length -le 0) {
        Fail "Artifact is empty: $($artifact.FullName)"
    }
}

if ([string]::IsNullOrWhiteSpace($StagingDirectory)) {
    $stagingParent = [IO.Path]::GetTempPath()
    $StagingDirectory = Join-Path $stagingParent ("AI-Studio-$ReleaseVersion-release-" + [guid]::NewGuid().ToString('N'))
} else {
    if (Test-Path -LiteralPath $StagingDirectory) {
        Fail "Refusing to reuse existing staging path; choose a new path to avoid overwriting release assets: $StagingDirectory"
    }
    $StagingDirectory = [IO.Path]::GetFullPath($StagingDirectory)
}

if (Test-PathWithin $StagingDirectory $RepositoryRoot) {
    Fail "Refusing to create staging inside the repository; this prevents release binaries from entering Git: $StagingDirectory"
}

New-Item -ItemType Directory -Path $StagingDirectory -Force | Out-Null

$copyMap = @(
    [pscustomobject]@{ Source = $msi.FullName; DestinationName = $MsiName }
    [pscustomobject]@{ Source = $nsis.FullName; DestinationName = $NsisName }
    [pscustomobject]@{ Source = $portable.FullName; DestinationName = $PortableName }
)

foreach ($item in $copyMap) {
    $destination = Join-Path $StagingDirectory $item.DestinationName
    Copy-Item -LiteralPath $item.Source -Destination $destination
    if (-not (Test-Path -LiteralPath $destination -PathType Leaf)) {
        Fail "Failed to stage artifact: $destination"
    }
}

$source = Get-SourceMetadata $RepositoryRoot
$manifestLines = @(
    "AI Studio $ReleaseVersion artifact checksums"
    "SOURCE_HEAD_SHA=$($source.Head)"
    "SOURCE_WORKTREE_STATUS=$($source.Worktree)"
    'ARTIFACT_SET_STATUS=STAGED'
    ''
)

foreach ($item in @($copyMap | Sort-Object DestinationName)) {
    $stagedPath = Join-Path $StagingDirectory $item.DestinationName
    $stagedFile = Get-Item -LiteralPath $stagedPath
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedPath).Hash.ToUpperInvariant()
    $manifestLines += "{0} | bytes={1} | SHA256={2}" -f $item.DestinationName, $stagedFile.Length, $hash
}

$manifestPath = Join-Path $StagingDirectory $ManifestName
Set-Content -LiteralPath $manifestPath -Value $manifestLines -Encoding UTF8

Verify-Staging $StagingDirectory

Write-Output ''
Write-Output "Release staging ready: $StagingDirectory"
Write-Output "Checksum manifest: $manifestPath"
Write-Output 'Independent recheck commands:'
foreach ($item in @($copyMap | Sort-Object DestinationName)) {
    $stagedPath = Join-Path $StagingDirectory $item.DestinationName
    $quotedPath = $stagedPath.Replace("'", "''")
    Write-Output "Get-FileHash -Algorithm SHA256 -LiteralPath '$quotedPath'"
}
Write-Output "Verify all staged assets: pwsh -NoProfile -File '$($PSCommandPath.Replace("'", "''"))' -Verify -StagingDirectory '$($StagingDirectory.Replace("'", "''"))'"
