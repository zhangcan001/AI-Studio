[CmdletBinding()]
param(
    [switch]$RunLiveComfy,
    [string]$LivePackageRoot,
    [string]$LogPath
)

# The live branch intentionally cannot report a DEV-062 PASS with the
# repository's current Rust live test: that test exercises ShotBatchService,
# not ProductionPackageService, and does not expose the package evidence
# contract. This script records that gap and returns BLOCKED after any
# optional reference smoke succeeds.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repoRoot "src-tauri/Cargo.toml"
$noGpuTestPath = Join-Path $repoRoot "src-tauri/tests/dev061b_production_package_hardening.rs"
$liveTestPath = Join-Path $repoRoot "src-tauri/tests/dev055_live_comfy.rs"
$blockedExitCode = 2

function Write-Record {
    param([string]$Message)

    $line = "[{0}] {1}" -f (Get-Date).ToString("o"), $Message
    Write-Host $line
    if ($LogPath) {
        Add-Content -LiteralPath $LogPath -Value $line
    }
}

function Stop-Blocked {
    param([Parameter(Mandatory = $true)][string]$Reason)

    Write-Record "DEV062_RESULT=BLOCKED reason=$Reason"
    exit $blockedExitCode
}

function Invoke-CargoTest {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    Write-Record "DEV062_GATE=$Label status=STARTED command=cargo $($Arguments -join ' ')"
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        # Native stderr is surfaced as ErrorRecords by PowerShell. Keep cargo
        # warnings non-terminating while preserving the native exit code.
        $ErrorActionPreference = "Continue"
        & cargo @Arguments 2>&1 | ForEach-Object {
            Write-Record ([string]$_)
        }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if ($exitCode -eq 0) {
        Write-Record "DEV062_GATE=$Label status=PASS"
    } else {
        Write-Record "DEV062_GATE=$Label status=FAIL exit_code=$exitCode"
    }
    return $exitCode
}

function Get-AbsoluteEnvironmentPath {
    param([Parameter(Mandatory = $true)][string]$Name)

    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value) -or -not [IO.Path]::IsPathRooted($value)) {
        return $null
    }
    return [IO.Path]::GetFullPath($value)
}

function Find-DiscoveredSettingsPath {
    $settingsCandidates = [System.Collections.Generic.List[string]]::new()
    $settingsFile = Get-AbsoluteEnvironmentPath "AI_STUDIO_LIVE_SETTINGS_FILE"
    if ($settingsFile) {
        [void]$settingsCandidates.Add((Join-Path (Split-Path -Parent $settingsFile) "settings.json"))
    }
    foreach ($name in @("AI_STUDIO_LIVE_DATA_ROOT", "AI_STUDIO_DATA_ROOT")) {
        $dataRoot = Get-AbsoluteEnvironmentPath $name
        if ($dataRoot) {
            [void]$settingsCandidates.Add((Join-Path $dataRoot "config/settings.json"))
        }
    }
    if ($env:LOCALAPPDATA) {
        [void]$settingsCandidates.Add((Join-Path $env:LOCALAPPDATA "AIStudio/AIStudioData/config/settings.json"))
    } elseif ($env:XDG_DATA_HOME) {
        [void]$settingsCandidates.Add((Join-Path $env:XDG_DATA_HOME "AIStudioData/config/settings.json"))
    } elseif ($env:USERPROFILE) {
        [void]$settingsCandidates.Add((Join-Path $env:USERPROFILE "AIStudioData/config/settings.json"))
    }

    return $settingsCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
}

function Test-LiveComfyConfiguration {
    $settingsPath = Find-DiscoveredSettingsPath
    if (-not $settingsPath) {
        return "settings.json_not_found_in_existing_test_discovery_locations"
    }

    $workflowCandidates = [System.Collections.Generic.List[string]]::new()
    $explicitWorkflowRoot = Get-AbsoluteEnvironmentPath "AI_STUDIO_LIVE_WORKFLOW_LIBRARY"
    if ($explicitWorkflowRoot) {
        [void]$workflowCandidates.Add($explicitWorkflowRoot)
    }
    foreach ($name in @("AI_STUDIO_LIVE_DATA_ROOT", "AI_STUDIO_DATA_ROOT")) {
        $dataRoot = Get-AbsoluteEnvironmentPath $name
        if ($dataRoot) {
            [void]$workflowCandidates.Add((Join-Path $dataRoot "workflow_library"))
        }
    }
    if ($env:LOCALAPPDATA) {
        [void]$workflowCandidates.Add((Join-Path $env:LOCALAPPDATA "AIStudio/AIStudioData/workflow_library"))
    } elseif ($env:XDG_DATA_HOME) {
        [void]$workflowCandidates.Add((Join-Path $env:XDG_DATA_HOME "AIStudioData/workflow_library"))
    } elseif ($env:USERPROFILE) {
        [void]$workflowCandidates.Add((Join-Path $env:USERPROFILE "AIStudioData/workflow_library"))
    }

    if (-not ($workflowCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Container })) {
        return "workflow_library_not_found_in_existing_test_discovery_locations"
    }
    return $null
}

function Get-LiveComfyEndpoint {
    param([Parameter(Mandatory = $true)][string]$SettingsPath)

    $override = [Environment]::GetEnvironmentVariable("AI_STUDIO_LIVE_COMFY_ENDPOINT")
    if (-not [string]::IsNullOrWhiteSpace($override)) {
        $rawEndpoint = $override.Trim()
        $source = "AI_STUDIO_LIVE_COMFY_ENDPOINT"
    } else {
        try {
            $settings = Get-Content -LiteralPath $SettingsPath -Raw | ConvertFrom-Json
        } catch {
            return "settings.json_is_not_valid_JSON"
        }
        try {
            $rawEndpoint = [string]$settings.comfy.endpoint
        } catch {
            return "settings.json_comfy_endpoint_is_unreadable"
        }
        $source = $SettingsPath
    }
    if ([string]::IsNullOrWhiteSpace($rawEndpoint)) {
        return "comfy.endpoint_is_missing"
    }

    try {
        $uri = [Uri]::new($rawEndpoint.Trim(), [UriKind]::Absolute)
    } catch {
        return "comfy.endpoint_is_not_an_absolute_URI"
    }
    if ($uri.Scheme -notin @("http", "https") -or $uri.Query -or $uri.Fragment -or $uri.UserInfo) {
        return "comfy.endpoint_must_be_http_or_https_without_query_fragment_or_credentials"
    }
    [pscustomobject]@{
        Uri = $uri
        Source = $source
    }
}

function Invoke-ComfyEndpointPreflight {
    param(
        [Parameter(Mandatory = $true)][Uri]$Endpoint,
        [Parameter(Mandatory = $true)][string]$Source
    )

    $baseEndpoint = $Endpoint.GetLeftPart([UriPartial]::Path).TrimEnd("/")
    $handler = [System.Net.Http.HttpClientHandler]::new()
    $client = $null
    try {
        $handler.UseProxy = $false
        $handler.Proxy = $null
        $client = [System.Net.Http.HttpClient]::new($handler)
        $client.Timeout = [TimeSpan]::FromSeconds(5)
        Write-Record "COMFY_PREFLIGHT=STARTED source=$Source proxy=disabled timeout_seconds=5"

        foreach ($path in @("/system_stats", "/object_info")) {
            $response = $null
            try {
                $response = $client.GetAsync("$baseEndpoint$path").GetAwaiter().GetResult()
                $statusCode = [int]$response.StatusCode
                if (-not $response.IsSuccessStatusCode) {
                    return "GET $path returned HTTP $statusCode"
                }
                Write-Record "COMFY_PREFLIGHT_GET=PASS path=$path status=$statusCode"
            } catch {
                return "GET $path failed: $($_.Exception.Message)"
            } finally {
                if ($response) {
                    $response.Dispose()
                }
            }
        }
        return $null
    } finally {
        if ($client) {
            $client.Dispose()
        }
        $handler.Dispose()
    }
}

function Find-DiscoveredWorkflowLibraryRoot {
    $candidates = [System.Collections.Generic.List[string]]::new()
    $explicitRoot = Get-AbsoluteEnvironmentPath "AI_STUDIO_LIVE_WORKFLOW_LIBRARY"
    if ($explicitRoot) {
        [void]$candidates.Add($explicitRoot)
    }
    foreach ($name in @("AI_STUDIO_LIVE_DATA_ROOT", "AI_STUDIO_DATA_ROOT")) {
        $dataRoot = Get-AbsoluteEnvironmentPath $name
        if ($dataRoot) {
            [void]$candidates.Add((Join-Path $dataRoot "workflow_library"))
        }
    }
    if ($env:LOCALAPPDATA) {
        [void]$candidates.Add((Join-Path $env:LOCALAPPDATA "AIStudio/AIStudioData/workflow_library"))
    } elseif ($env:XDG_DATA_HOME) {
        [void]$candidates.Add((Join-Path $env:XDG_DATA_HOME "AIStudioData/workflow_library"))
    } elseif ($env:USERPROFILE) {
        [void]$candidates.Add((Join-Path $env:USERPROFILE "AIStudioData/workflow_library"))
    }

    return $candidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Container } |
        Select-Object -First 1
}

function Find-DynamicH3ImageToVideoPackages {
    param([Parameter(Mandatory = $true)][string]$Root)

    foreach ($directory in Get-ChildItem -LiteralPath $Root -Directory) {
        $manifestPath = Join-Path $directory.FullName "manifest.yaml"
        $recipePath = Join-Path $directory.FullName "recipe.yaml"
        if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or
            -not (Test-Path -LiteralPath $recipePath -PathType Leaf)) {
            continue
        }

        $manifest = Get-Content -LiteralPath $manifestPath -Raw
        if ($manifest -notmatch '(?m)^\s*mode:\s*fl2va_image_to_video\s*$') {
            continue
        }
        $manifestId = if ($manifest -match '(?m)^\s*id:\s*(\S+)\s*$') { $Matches[1] } else { $null }
        $workflowVersion = if ($manifest -match '(?m)^\s*workflow_version:\s*(\S+)\s*$') { $Matches[1] } else { $null }
        $recipe = Get-Content -LiteralPath $recipePath -Raw
        $recipeDeclaredId = if ($recipe -match '(?m)^\s*id:\s*(\S+)\s*$') { $Matches[1] } else { $null }
        if ($manifestId -and $workflowVersion -and $recipeDeclaredId) {
            [pscustomobject]@{
                packagePath = $directory.FullName
                manifestId = $manifestId
                workflowVersion = $workflowVersion
                recipeDeclaredId = $recipeDeclaredId
            }
        }
    }
}

function Test-SupportedImageFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }
    $bytes = @(Get-Content -LiteralPath $Path -Encoding Byte -TotalCount 12)
    if ($bytes.Count -lt 3) {
        return $false
    }
    $signature = ($bytes | ForEach-Object { $_.ToString("X2") }) -join ""
    return $signature.StartsWith("89504E470D0A1A0A") -or
        $signature.StartsWith("FFD8FF") -or
        ($signature.StartsWith("52494646") -and $signature.Length -ge 24 -and $signature.Substring(16, 8) -eq "57454250")
}

function Test-ProductionPackageFixture {
    param([Parameter(Mandatory = $true)][string]$Root)

    if (-not [IO.Path]::IsPathRooted($Root)) {
        return "package root must be an absolute path"
    }
    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        return "package root does not exist"
    }
    $rootFullPath = [IO.Path]::GetFullPath($Root)
    $manifestPath = Join-Path $rootFullPath "production-package.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        return "production-package.json is missing"
    }

    try {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    } catch {
        return "production-package.json is not valid JSON"
    }
    if ($manifest.schemaVersion -ne 1) {
        return "schemaVersion must be 1"
    }
    if ($manifest.packageType -ne "AI_STUDIO_VIDEO_PRODUCTION") {
        return "packageType is not AI_STUDIO_VIDEO_PRODUCTION"
    }
    $items = @($manifest.items)
    if ($items.Count -ne 1) {
        return "DEV-062 live fixture must contain exactly one item; found $($items.Count)"
    }
    $item = $items[0]
    if ([string]::IsNullOrWhiteSpace([string]$item.videoPrompt)) {
        return "the single item must have a non-empty videoPrompt"
    }
    $firstFrame = [string]$item.firstFrame
    if ([string]::IsNullOrWhiteSpace($firstFrame)) {
        return "the single item must provide firstFrame as the source image"
    }
    if ([IO.Path]::IsPathRooted($firstFrame) -or $firstFrame -match '(^|[\\/])\.\.([\\/]|$)' -or $firstFrame -match '^[a-zA-Z][a-zA-Z0-9+.-]*://') {
        return "firstFrame must be a package-relative local path"
    }
    $mediaPath = [IO.Path]::GetFullPath((Join-Path $rootFullPath $firstFrame))
    $rootPrefix = $rootFullPath.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $mediaPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        return "firstFrame resolves outside the package root"
    }
    if (-not (Test-SupportedImageFile -Path $mediaPath)) {
        return "firstFrame is missing or has no supported PNG/JPEG/WebP signature"
    }
    return $null
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    Stop-Blocked "Rust manifest not found at $manifestPath"
}
if (-not (Test-Path -LiteralPath $noGpuTestPath -PathType Leaf)) {
    Stop-Blocked "canonical 500-item no-GPU test not found at $noGpuTestPath"
}
if ($RunLiveComfy -and -not (Test-Path -LiteralPath $liveTestPath -PathType Leaf)) {
    Stop-Blocked "canonical real ComfyUI test not found at $liveTestPath"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Stop-Blocked "cargo is not available on PATH"
}

if ($LogPath) {
    $logParent = Split-Path -Parent $LogPath
    if ($logParent -and -not (Test-Path -LiteralPath $logParent -PathType Container)) {
        New-Item -ItemType Directory -Path $logParent -Force | Out-Null
    }
}

if (-not $LivePackageRoot) {
    $LivePackageRoot = [Environment]::GetEnvironmentVariable("AI_STUDIO_DEV062_PACKAGE_ROOT")
}

Write-Record "DEV062_SMOKE=STARTED repo=$repoRoot live_requested=$RunLiveComfy"
Write-Record "DEV062_POLICY=no_auto_start no_direct_comfy_call no_hardcoded_workflow_identifiers existing_package_queue_paths_only"

$noGpuArguments = @(
    "test",
    "--locked",
    "--manifest-path", $manifestPath,
    "--test", "dev061b_production_package_hardening",
    "complete_500_item_package_is_five_frozen_batches_without_tasks_or_comfy_submit",
    "--",
    "--exact",
    "--nocapture"
)
$noGpuExitCode = Invoke-CargoTest -Label "NO_GPU_500_ITEM" -Arguments $noGpuArguments
if ($noGpuExitCode -ne 0) {
    exit $noGpuExitCode
}

if (-not $RunLiveComfy) {
    Write-Record "DEV062_GATE=LIVE_COMFY status=BLOCKED reason=not_requested explicit_-RunLiveComfy_required"
    Write-Record "DEV062_RESULT=PASS no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit 0
}

if ($env:AI_STUDIO_LIVE_COMFY -ne "1") {
    Write-Record "DEV062_GATE=LIVE_COMFY status=BLOCKED reason=AI_STUDIO_LIVE_COMFY_must_equal_1"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}

$liveConfigurationReason = Test-LiveComfyConfiguration
if ($liveConfigurationReason) {
    Write-Record "DEV062_GATE=LIVE_COMFY status=BLOCKED reason=$liveConfigurationReason"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}

$settingsPath = Find-DiscoveredSettingsPath
$endpointResolution = Get-LiveComfyEndpoint -SettingsPath $settingsPath
if ($endpointResolution -is [string]) {
    Write-Record "COMFY_PREFLIGHT=BLOCKED reason=$endpointResolution"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}

try {
    $preflightReason = Invoke-ComfyEndpointPreflight `
        -Endpoint $endpointResolution.Uri `
        -Source $endpointResolution.Source
} catch {
    $preflightReason = "preflight_failed: $($_.Exception.Message)"
}
if ($preflightReason) {
    Write-Record "COMFY_PREFLIGHT=BLOCKED endpoint=$($endpointResolution.Uri.AbsoluteUri) reason=$preflightReason"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}
Write-Record "COMFY_PREFLIGHT=PASS endpoint=$($endpointResolution.Uri.AbsoluteUri) checks=/system_stats,/object_info"

if (-not $LivePackageRoot) {
    Write-Record "DEV062_GATE=LIVE_PACKAGE_FIXTURE status=BLOCKED reason=provide_-LivePackageRoot_or_AI_STUDIO_DEV062_PACKAGE_ROOT"
    Write-Record "DEV062_EVIDENCE=INSPECT=BLOCKED CREATE=BLOCKED MANUAL_QUEUE_START=BLOCKED VIDEO_ASSET=BLOCKED"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}

$fixtureReason = Test-ProductionPackageFixture -Root $LivePackageRoot
if ($fixtureReason) {
    Write-Record "DEV062_GATE=LIVE_PACKAGE_FIXTURE status=BLOCKED reason=$fixtureReason"
    Write-Record "DEV062_EVIDENCE=INSPECT=BLOCKED CREATE=BLOCKED MANUAL_QUEUE_START=BLOCKED VIDEO_ASSET=BLOCKED"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}
Write-Record "DEV062_GATE=LIVE_PACKAGE_FIXTURE status=PASS root=$([IO.Path]::GetFullPath($LivePackageRoot)) item_count=1 source_image=PASS"

$workflowLibraryRoot = Find-DiscoveredWorkflowLibraryRoot
if (-not $workflowLibraryRoot) {
    Write-Record "DEV062_GATE=DYNAMIC_WORKFLOW_DISCOVERY status=BLOCKED reason=workflow_library_root_not_found"
    Write-Record "LIVE_WORKFLOW_VERSION_ID=UNAVAILABLE reason=existing_live_test_does_not_expose_package_database_ids"
    Write-Record "LIVE_RECIPE_ID=UNAVAILABLE reason=existing_live_test_does_not_expose_package_database_ids"
    Write-Record "DEV062_EVIDENCE=INSPECT=BLOCKED CREATE=BLOCKED MANUAL_QUEUE_START=BLOCKED VIDEO_ASSET=BLOCKED"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}
$dynamicH3Packages = @(Find-DynamicH3ImageToVideoPackages -Root $workflowLibraryRoot)
if ($dynamicH3Packages.Count -eq 0) {
    Write-Record "DEV062_GATE=DYNAMIC_WORKFLOW_DISCOVERY status=BLOCKED reason=no_fl2va_image_to_video_runtime_package_found root=$workflowLibraryRoot"
    Write-Record "LIVE_WORKFLOW_VERSION_ID=UNAVAILABLE reason=no_dynamic_h3_candidate"
    Write-Record "LIVE_RECIPE_ID=UNAVAILABLE reason=no_dynamic_h3_candidate"
    Write-Record "DEV062_EVIDENCE=INSPECT=BLOCKED CREATE=BLOCKED MANUAL_QUEUE_START=BLOCKED VIDEO_ASSET=BLOCKED"
    Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy=BLOCKED"
    exit $blockedExitCode
}
foreach ($candidate in $dynamicH3Packages) {
    Write-Record "DEV062_DYNAMIC_CATALOG_CANDIDATE package=$($candidate.packagePath) manifest_id=$($candidate.manifestId) workflow_version=$($candidate.workflowVersion) recipe_declared_id=$($candidate.recipeDeclaredId)"
}
Write-Record "DEV062_GATE=DYNAMIC_WORKFLOW_DISCOVERY status=PARTIAL candidates=$($dynamicH3Packages.Count) root=$workflowLibraryRoot"
Write-Record "LIVE_WORKFLOW_VERSION_ID=UNAVAILABLE reason=workflow_library_sync_generates_database_id_and_existing_dev055_test_does_not_emit_it"
Write-Record "LIVE_RECIPE_ID=UNAVAILABLE reason=workflow_library_sync_generates_database_id_and_existing_dev055_test_does_not_emit_it"
Write-Record "DEV062_GATE=LIVE_PACKAGE_PIPELINE status=BLOCKED reason=existing_dev055_entry_is_shot_batch_not_production_package_and_does_not_consume_fixture_or_emit_package_evidence"
Write-Record "DEV062_EVIDENCE=INSPECT=BLOCKED CREATE=BLOCKED MANUAL_QUEUE_START=BLOCKED VIDEO_ASSET=BLOCKED"

Write-Record "DEV062_LIVE_COMFY_POLICY=explicit_opt_in_only reference_test_only automatic_process_start_disabled no_dev062_package_pass_claim"
$liveArguments = @(
    "test",
    "--locked",
    "--manifest-path", $manifestPath,
    "--test", "dev055_live_comfy",
    "dev055_real_comfyui_release_gate",
    "--",
    "--ignored",
    "--exact",
    "--nocapture"
)
$liveExitCode = Invoke-CargoTest -Label "LIVE_COMFY_REFERENCE_ONLY" -Arguments $liveArguments
if ($liveExitCode -ne 0) {
    Write-Record "DEV062_RESULT=FAIL no_gpu_500_item=PASS live_comfy_reference=FAIL live_package=BLOCKED"
    exit $liveExitCode
}

Write-Record "DEV062_GATE=LIVE_COMFY_REFERENCE_ONLY status=PASS evidence=existing_dev055_queue_path_only"
Write-Record "DEV062_RESULT=BLOCKED no_gpu_500_item=PASS live_comfy_reference=PASS live_package=BLOCKED"
exit $blockedExitCode
