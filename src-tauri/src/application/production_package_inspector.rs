//! Zero-write inspection for an external Production Package V1.
//!
//! This module deliberately stops at an inspection snapshot.  It does not
//! import assets, create batches, call ComfyUI, or trust workflow identifiers
//! supplied by an external package.  The package root and every referenced
//! media path are canonicalized before any media bytes are accepted.

use crate::application::h3_local_import_service::is_supported_h3_output_resolution;
use crate::application::media_probe::{AudioMetadata, CommandMediaProbe, MediaProbe};
use crate::application::source_asset_import_service::{
    MAX_SOURCE_AUDIO_BYTES, MAX_SOURCE_IMAGE_BYTES, MAX_SOURCE_VIDEO_BYTES,
};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::io::AsyncReadExt;

pub const PRODUCTION_PACKAGE_SCHEMA_VERSION: u32 = 1;
pub const PRODUCTION_PACKAGE_TYPE: &str = "AI_STUDIO_VIDEO_PRODUCTION";
pub const MAX_PRODUCTION_PACKAGE_ITEMS: usize = 500;
pub const MAX_PACKAGE_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_PACKAGE_PROMPT_PREVIEW_CHARS: usize = 300;
pub const MAX_PACKAGE_JSON_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_PACKAGE_DURATION_SECONDS: i64 = 5;
pub const DEFAULT_PACKAGE_WIDTH: i64 = 960;
pub const DEFAULT_PACKAGE_HEIGHT: i64 = 544;

const PACKAGE_EXECUTION_FIELDS: &[&str] = &[
    "workflowVersionId",
    "recipeId",
    "taskId",
    "batchId",
    "assetId",
    "comfyPromptId",
    "selectedVideoAssetId",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProductionPackageItemStatus {
    Ready,
    Warning,
    Blocked,
}

impl ProductionPackageItemStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Warning => "WARNING",
            Self::Blocked => "BLOCKED",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProductionPackageDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: ProductionPackageDiagnosticSeverity,
    pub field: Option<String>,
}

impl ProductionPackageDiagnostic {
    fn warning(code: impl Into<String>, message: impl Into<String>, field: Option<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ProductionPackageDiagnosticSeverity::Warning,
            field,
        }
    }

    pub(crate) fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        field: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ProductionPackageDiagnosticSeverity::Error,
            field,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDefaultsInspection {
    pub duration_seconds: i64,
    pub width: i64,
    pub height: i64,
    pub mode: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageMediaInspection {
    pub relative_path: String,
    pub kind: String,
    pub exists: bool,
    pub regular_file: bool,
    pub readable: bool,
    pub format: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing)]
    pub(crate) resolved_path: Option<PathBuf>,
}

impl ProductionPackageMediaInspection {
    fn empty(relative_path: &str, kind: MediaKind) -> Self {
        Self {
            relative_path: relative_path.to_owned(),
            kind: kind.as_str().to_owned(),
            exists: false,
            regular_file: false,
            readable: false,
            format: None,
            mime_type: None,
            size_bytes: None,
            sha256: None,
            width: None,
            height: None,
            duration_ms: None,
            resolved_path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageItemInspection {
    pub id: String,
    pub name: String,
    pub text: Option<String>,
    pub image_prompt: Option<String>,
    pub video_prompt: String,
    pub video_prompt_preview: String,
    pub episode: Option<String>,
    pub scene: Option<String>,
    pub mode: String,
    pub duration_seconds: i64,
    pub width: i64,
    pub height: i64,
    pub first_frame: Option<ProductionPackageMediaInspection>,
    pub last_frame: Option<ProductionPackageMediaInspection>,
    pub reference_images: Vec<ProductionPackageMediaInspection>,
    pub reference_audios: Vec<ProductionPackageMediaInspection>,
    pub reference_videos: Vec<ProductionPackageMediaInspection>,
    /// Non-persistent project-aware production preflight fields. The pure
    /// inspector leaves these unset; ProductionPackageService enriches them.
    pub resolved_workflow_version_id: Option<String>,
    pub resolved_recipe_id: Option<String>,
    pub workflow_resolution_source: Option<String>,
    pub recipe_compatibility: Option<String>,
    pub status: ProductionPackageItemStatus,
    pub warnings: Vec<ProductionPackageDiagnostic>,
    pub errors: Vec<ProductionPackageDiagnostic>,
}

impl ProductionPackageItemInspection {
    pub fn is_ready(&self) -> bool {
        self.status == ProductionPackageItemStatus::Ready
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageInspection {
    pub package_name: String,
    pub package_id: Option<String>,
    pub package_type: String,
    pub defaults: ProductionPackageDefaultsInspection,
    pub manifest_sha256: String,
    pub item_count: usize,
    pub ready_count: usize,
    pub warning_count: usize,
    pub blocked_count: usize,
    pub status: ProductionPackageItemStatus,
    pub items: Vec<ProductionPackageItemInspection>,
    pub warnings: Vec<ProductionPackageDiagnostic>,
    pub errors: Vec<ProductionPackageDiagnostic>,
    #[serde(skip_serializing)]
    pub(crate) package_root: PathBuf,
}

impl ProductionPackageInspection {
    pub fn ready_items(&self) -> impl Iterator<Item = &ProductionPackageItemInspection> {
        self.items.iter().filter(|item| item.is_ready())
    }

    pub(crate) fn recompute_summary(&mut self) {
        self.item_count = self.items.len();
        self.ready_count = self
            .items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Ready)
            .count();
        self.warning_count = self.warnings.len()
            + self
                .items
                .iter()
                .filter(|item| item.status == ProductionPackageItemStatus::Warning)
                .count();
        self.blocked_count = self
            .items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Blocked)
            .count();
        self.status = if self.blocked_count > 0 {
            ProductionPackageItemStatus::Blocked
        } else if self.warning_count > 0 {
            ProductionPackageItemStatus::Warning
        } else {
            ProductionPackageItemStatus::Ready
        };
    }

    /// Adapt the richer application snapshot to the stable domain DTO used
    /// by commands and other application services.  The filesystem path is
    /// intentionally not copied into the domain response.
    pub fn to_domain_inspection(
        &self,
    ) -> crate::domain::production_package::ProductionPackageInspection {
        use crate::domain::production_package as domain;

        let items = self
            .items
            .iter()
            .map(|item| domain::ProductionPackageInspectionItem {
                id: item.id.clone(),
                name: item.name.clone(),
                mode: Some(item.mode.clone()),
                video_prompt_preview: Some(item.video_prompt_preview.clone()),
                first_frame: item.first_frame.as_ref().map(to_domain_media),
                last_frame: item.last_frame.as_ref().map(to_domain_media),
                references: item
                    .reference_images
                    .iter()
                    .chain(item.reference_audios.iter())
                    .chain(item.reference_videos.iter())
                    .map(to_domain_media)
                    .collect(),
                duration_seconds: Some(item.duration_seconds),
                width: Some(item.width),
                height: Some(item.height),
                resolved_workflow_version_id: item.resolved_workflow_version_id.clone(),
                resolved_recipe_id: item.resolved_recipe_id.clone(),
                workflow_resolution_source: item.workflow_resolution_source.clone(),
                recipe_compatibility: item.recipe_compatibility.clone(),
                status: match item.status {
                    ProductionPackageItemStatus::Ready => {
                        domain::ProductionPackageItemStatus::Ready
                    }
                    ProductionPackageItemStatus::Warning => {
                        domain::ProductionPackageItemStatus::Warning
                    }
                    ProductionPackageItemStatus::Blocked => {
                        domain::ProductionPackageItemStatus::Blocked
                    }
                },
                warnings: item.warnings.iter().map(to_domain_diagnostic).collect(),
                errors: item.errors.iter().map(to_domain_diagnostic).collect(),
            })
            .collect();
        domain::ProductionPackageInspection::from_items(
            self.package_name.clone(),
            self.package_id.clone(),
            items,
            self.warnings.iter().map(to_domain_diagnostic).collect(),
            self.errors.iter().map(to_domain_diagnostic).collect(),
        )
    }
}

fn to_domain_media(
    media: &ProductionPackageMediaInspection,
) -> crate::domain::production_package::ProductionPackageMediaMetadata {
    use crate::domain::production_package::{
        ProductionPackageMediaKind, ProductionPackageMediaMetadata,
    };

    ProductionPackageMediaMetadata {
        relative_path: media.relative_path.clone(),
        kind: match media.kind.as_str() {
            "image" => ProductionPackageMediaKind::Image,
            "video" => ProductionPackageMediaKind::Video,
            "audio" => ProductionPackageMediaKind::Audio,
            _ => ProductionPackageMediaKind::Unknown,
        },
        exists: media.exists,
        is_file: media.regular_file,
        readable: media.readable,
        format_supported: media.format.is_some(),
        size_bytes: media.size_bytes,
        sha256: media.sha256.clone(),
        mime_type: media.mime_type.clone(),
        width: media.width.map(i64::from),
        height: media.height.map(i64::from),
        duration_ms: media.duration_ms,
    }
}

fn to_domain_diagnostic(
    diagnostic: &ProductionPackageDiagnostic,
) -> crate::domain::production_package::ProductionPackageDiagnostic {
    use self::ProductionPackageDiagnosticSeverity as InspectorSeverity;
    use crate::domain::production_package::{
        ProductionPackageDiagnostic as DomainDiagnostic,
        ProductionPackageDiagnosticSeverity as DomainSeverity,
    };

    let severity = match diagnostic.severity {
        InspectorSeverity::Warning => DomainSeverity::Warning,
        InspectorSeverity::Error => DomainSeverity::Error,
    };
    DomainDiagnostic {
        severity,
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        field: diagnostic.field.clone(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductionPackageInspectionError {
    PackagePathInvalid {
        message: String,
    },
    PackageJsonMissing,
    PackageJsonUnreadable {
        message: String,
    },
    PackageJsonTooLarge {
        max_bytes: u64,
        actual_bytes: u64,
    },
    PackageJsonInvalid {
        message: String,
    },
    PackageSchemaUnsupported {
        message: String,
    },
    PackageTypeUnsupported {
        message: String,
    },
    PackageEmpty,
    PackageTooLarge {
        max_items: usize,
        actual_items: usize,
    },
    PackageMediaMissing,
    PackageMediaInvalid {
        message: String,
    },
    PackageMediaChanged,
}

impl ProductionPackageInspectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PackagePathInvalid { .. } => "PACKAGE_PATH_INVALID",
            Self::PackageJsonMissing
            | Self::PackageJsonUnreadable { .. }
            | Self::PackageJsonInvalid { .. }
            | Self::PackageJsonTooLarge { .. } => "PACKAGE_JSON_INVALID",
            Self::PackageSchemaUnsupported { .. } => "PACKAGE_SCHEMA_UNSUPPORTED",
            Self::PackageTypeUnsupported { .. } => "PACKAGE_TYPE_UNSUPPORTED",
            Self::PackageEmpty => "PACKAGE_EMPTY",
            Self::PackageTooLarge { .. } => "PACKAGE_TOO_LARGE",
            Self::PackageMediaMissing => "PACKAGE_MEDIA_MISSING",
            Self::PackageMediaInvalid { .. } => "PACKAGE_MEDIA_INVALID",
            Self::PackageMediaChanged => "PACKAGE_MEDIA_CHANGED",
        }
    }
}

impl fmt::Display for ProductionPackageInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackagePathInvalid { message }
            | Self::PackageJsonInvalid { message }
            | Self::PackageSchemaUnsupported { message }
            | Self::PackageTypeUnsupported { message } => {
                write!(formatter, "{}: {message}", self.code())
            }
            Self::PackageJsonMissing => {
                write!(
                    formatter,
                    "{}: production-package.json is missing",
                    self.code()
                )
            }
            Self::PackageJsonUnreadable { message } => {
                write!(formatter, "{}: {message}", self.code())
            }
            Self::PackageJsonTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "{}: manifest is {actual_bytes} bytes; maximum is {max_bytes}",
                self.code()
            ),
            Self::PackageEmpty => write!(formatter, "{}: package contains no items", self.code()),
            Self::PackageTooLarge {
                max_items,
                actual_items,
            } => write!(
                formatter,
                "{}: package contains {actual_items} items; maximum is {max_items}",
                self.code()
            ),
            Self::PackageMediaMissing => {
                write!(formatter, "{}: media is missing", self.code())
            }
            Self::PackageMediaInvalid { message } => {
                write!(formatter, "{}: {message}", self.code())
            }
            Self::PackageMediaChanged => {
                write!(formatter, "{}: media changed after inspection", self.code())
            }
        }
    }
}

impl Error for ProductionPackageInspectionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MediaKind {
    Image,
    Audio,
    Video,
}

impl MediaKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }

    const fn max_bytes(self) -> u64 {
        match self {
            Self::Image => MAX_SOURCE_IMAGE_BYTES,
            Self::Audio => MAX_SOURCE_AUDIO_BYTES,
            Self::Video => MAX_SOURCE_VIDEO_BYTES,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ParsedDefaults {
    duration_seconds: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    mode: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ParsedItem {
    id: String,
    name: String,
    text: Option<String>,
    image_prompt: Option<String>,
    video_prompt: String,
    episode: Option<String>,
    scene: Option<String>,
    mode: Option<String>,
    duration_seconds: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    first_frame: Option<String>,
    last_frame: Option<String>,
    reference_images: Vec<String>,
    reference_audios: Vec<String>,
    reference_videos: Vec<String>,
    warnings: Vec<ProductionPackageDiagnostic>,
    errors: Vec<ProductionPackageDiagnostic>,
}

pub struct ProductionPackageInspector {
    media_probe: Arc<dyn MediaProbe>,
}

impl Default for ProductionPackageInspector {
    fn default() -> Self {
        Self {
            media_probe: Arc::new(CommandMediaProbe::default()),
        }
    }
}

impl ProductionPackageInspector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_media_probe(mut self, media_probe: Arc<dyn MediaProbe>) -> Self {
        self.media_probe = media_probe;
        self
    }

    pub async fn inspect(
        &self,
        package_root: impl AsRef<Path>,
    ) -> Result<ProductionPackageInspection, ProductionPackageInspectionError> {
        let package_root = canonical_directory(package_root.as_ref())?;
        let manifest_path = package_root.join("production-package.json");
        let manifest_path = canonical_manifest_path(&package_root, &manifest_path)?;
        let metadata = fs::metadata(&manifest_path).map_err(|error| {
            ProductionPackageInspectionError::PackageJsonUnreadable {
                message: error.to_string(),
            }
        })?;
        if metadata.len() > MAX_PACKAGE_JSON_BYTES {
            return Err(ProductionPackageInspectionError::PackageJsonTooLarge {
                max_bytes: MAX_PACKAGE_JSON_BYTES,
                actual_bytes: metadata.len(),
            });
        }
        let bytes = tokio::fs::read(&manifest_path).await.map_err(|error| {
            ProductionPackageInspectionError::PackageJsonUnreadable {
                message: error.to_string(),
            }
        })?;
        let manifest_sha256 = sha256(&bytes);
        let document = serde_json::from_slice::<Value>(&bytes).map_err(|error| {
            ProductionPackageInspectionError::PackageJsonInvalid {
                message: format!("manifest is not valid JSON: {error}"),
            }
        })?;
        let root = document.as_object().ok_or_else(|| {
            ProductionPackageInspectionError::PackageJsonInvalid {
                message: "manifest root must be a JSON object".to_owned(),
            }
        })?;

        inspect_root_schema(root)?;
        let package_name = required_non_empty_string(root, "name", "package name")?;
        let package_id = optional_string(root, "packageId")?;
        let defaults = parse_defaults(root.get("defaults"))?;
        let mut package_warnings = unknown_fields(
            root,
            &[
                "schemaVersion",
                "packageType",
                "packageId",
                "name",
                "description",
                "createdBy",
                "createdAt",
                "source",
                "defaults",
                "items",
            ],
            "",
        );
        if let Some(defaults_object) = root.get("defaults").and_then(Value::as_object) {
            package_warnings.extend(unknown_fields(
                defaults_object,
                &["durationSeconds", "width", "height", "mode"],
                "defaults",
            ));
        }
        validate_optional_metadata(root, "description")?;
        validate_optional_metadata(root, "createdBy")?;
        validate_optional_metadata(root, "createdAt")?;
        validate_optional_metadata(root, "source")?;

        let items = root.get("items").ok_or_else(|| {
            ProductionPackageInspectionError::PackageJsonInvalid {
                message: "items is required".to_owned(),
            }
        })?;
        let items = items.as_array().ok_or_else(|| {
            ProductionPackageInspectionError::PackageJsonInvalid {
                message: "items must be an array".to_owned(),
            }
        })?;
        if items.is_empty() {
            return Err(ProductionPackageInspectionError::PackageEmpty);
        }
        if items.len() > MAX_PRODUCTION_PACKAGE_ITEMS {
            return Err(ProductionPackageInspectionError::PackageTooLarge {
                max_items: MAX_PRODUCTION_PACKAGE_ITEMS,
                actual_items: items.len(),
            });
        }

        let mut parsed_items = Vec::with_capacity(items.len());
        let mut id_counts = HashMap::<String, usize>::new();
        for (index, value) in items.iter().enumerate() {
            let parsed = parse_item(value, index)?;
            *id_counts.entry(parsed.id.clone()).or_default() += 1;
            parsed_items.push(parsed);
        }

        let mut inspected_items = Vec::with_capacity(parsed_items.len());
        for parsed in parsed_items {
            let duplicate = id_counts.get(&parsed.id).copied().unwrap_or_default() > 1;
            inspected_items.push(
                self.inspect_item(&package_root, &defaults, parsed, duplicate)
                    .await,
            );
        }

        let ready_count = inspected_items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Ready)
            .count();
        let warning_count = package_warnings.len()
            + inspected_items
                .iter()
                .filter(|item| item.status == ProductionPackageItemStatus::Warning)
                .count();
        let blocked_count = inspected_items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Blocked)
            .count();
        let status = if blocked_count > 0 {
            ProductionPackageItemStatus::Blocked
        } else if warning_count > 0 {
            ProductionPackageItemStatus::Warning
        } else {
            ProductionPackageItemStatus::Ready
        };
        let defaults_for_output = effective_defaults(&defaults);
        let package_errors = Vec::new();

        Ok(ProductionPackageInspection {
            package_name,
            package_id,
            package_type: PRODUCTION_PACKAGE_TYPE.to_owned(),
            defaults: defaults_for_output,
            manifest_sha256,
            item_count: inspected_items.len(),
            ready_count,
            warning_count,
            blocked_count,
            status,
            items: inspected_items,
            warnings: package_warnings,
            errors: package_errors,
            package_root,
        })
    }

    pub async fn inspect_package(
        &self,
        package_root: impl AsRef<Path>,
    ) -> Result<ProductionPackageInspection, ProductionPackageInspectionError> {
        self.inspect(package_root).await
    }

    pub async fn revalidate_media(
        &self,
        package_root: impl AsRef<Path>,
        media: &ProductionPackageMediaInspection,
    ) -> Result<(), ProductionPackageInspectionError> {
        let root = canonical_directory(package_root.as_ref())?;
        let relative = validate_relative_media_path(&media.relative_path)
            .map_err(|message| ProductionPackageInspectionError::PackagePathInvalid { message })?;
        let resolved = canonical_media_path(&root, &relative).map_err(|error| match error {
            MediaPathError::Missing => ProductionPackageInspectionError::PackageMediaMissing,
            MediaPathError::OutsideRoot(message) | MediaPathError::Invalid(message) => {
                ProductionPackageInspectionError::PackagePathInvalid { message }
            }
            MediaPathError::NotRegularFile(message) | MediaPathError::Unreadable(message) => {
                ProductionPackageInspectionError::PackageMediaInvalid { message }
            }
        })?;
        let metadata = fs::metadata(&resolved).map_err(|error| {
            ProductionPackageInspectionError::PackageMediaInvalid {
                message: error.to_string(),
            }
        })?;
        let current_hash = hash_file(&resolved)
            .await
            .map_err(|message| ProductionPackageInspectionError::PackageMediaInvalid { message })?;
        if media.size_bytes != Some(metadata.len())
            || media.sha256.as_deref() != Some(&current_hash)
        {
            return Err(ProductionPackageInspectionError::PackageMediaChanged);
        }
        Ok(())
    }

    async fn inspect_item(
        &self,
        package_root: &Path,
        defaults: &ParsedDefaults,
        parsed: ParsedItem,
        duplicate_id: bool,
    ) -> ProductionPackageItemInspection {
        let mut warnings = parsed.warnings.clone();
        let mut errors = parsed.errors.clone();
        if duplicate_id {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_DUPLICATE_ITEM_ID",
                "item id must be unique within the package",
                Some("id".to_owned()),
            ));
        }
        if parsed.video_prompt.trim().is_empty() {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_PROMPT_EMPTY",
                "videoPrompt must not be empty",
                Some("videoPrompt".to_owned()),
            ));
        } else if parsed.video_prompt.len() > MAX_PACKAGE_PROMPT_BYTES {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_PROMPT_TOO_LARGE",
                format!(
                    "videoPrompt exceeds the {} byte limit",
                    MAX_PACKAGE_PROMPT_BYTES
                ),
                Some("videoPrompt".to_owned()),
            ));
        }

        let raw_mode = parsed.mode.as_deref().or(defaults.mode.as_deref());
        let mode = match raw_mode {
            Some(raw_mode) => canonical_mode(raw_mode),
            None => infer_mode(&parsed),
        };
        let mode = match mode {
            Some(mode) => mode.to_owned(),
            None => {
                errors.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_MODE_UNSUPPORTED",
                    "mode is not supported by the current H3 recipes",
                    Some("mode".to_owned()),
                ));
                "UNSUPPORTED".to_owned()
            }
        };

        let duration_seconds = parsed
            .duration_seconds
            .or(defaults.duration_seconds)
            .unwrap_or(DEFAULT_PACKAGE_DURATION_SECONDS);
        if !(1..=15).contains(&duration_seconds) {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_DURATION_INVALID",
                "durationSeconds must be an integer from 1 to 15",
                Some("durationSeconds".to_owned()),
            ));
        }

        let width = parsed
            .width
            .or(defaults.width)
            .unwrap_or(DEFAULT_PACKAGE_WIDTH);
        let height = parsed
            .height
            .or(defaults.height)
            .unwrap_or(DEFAULT_PACKAGE_HEIGHT);
        if !is_supported_h3_output_resolution(width, height) {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_RESOLUTION_UNSUPPORTED",
                "resolution is not supported by the current H3 recipes",
                Some("resolution".to_owned()),
            ));
        }

        let first_frame = match parsed.first_frame.as_deref() {
            Some(path) => {
                let (media, diagnostics) = self
                    .inspect_media(package_root, path, MediaKind::Image)
                    .await;
                append_media_diagnostics(&mut warnings, &mut errors, diagnostics);
                Some(media)
            }
            None => None,
        };
        let last_frame = match parsed.last_frame.as_deref() {
            Some(path) => {
                let (media, diagnostics) = self
                    .inspect_media(package_root, path, MediaKind::Image)
                    .await;
                append_media_diagnostics(&mut warnings, &mut errors, diagnostics);
                Some(media)
            }
            None => None,
        };
        let reference_images = self
            .inspect_media_list(
                package_root,
                &parsed.reference_images,
                MediaKind::Image,
                &mut warnings,
                &mut errors,
            )
            .await;
        let reference_audios = self
            .inspect_media_list(
                package_root,
                &parsed.reference_audios,
                MediaKind::Audio,
                &mut warnings,
                &mut errors,
            )
            .await;
        let reference_videos = self
            .inspect_media_list(
                package_root,
                &parsed.reference_videos,
                MediaKind::Video,
                &mut warnings,
                &mut errors,
            )
            .await;

        validate_mode_inputs(
            &mode,
            first_frame.is_some(),
            last_frame.is_some(),
            !reference_images.is_empty(),
            !reference_audios.is_empty(),
            !reference_videos.is_empty(),
            &mut errors,
        );
        warn_about_unused_media(
            &mode,
            first_frame.is_some(),
            last_frame.is_some(),
            !reference_images.is_empty(),
            !reference_audios.is_empty(),
            !reference_videos.is_empty(),
            &mut warnings,
        );

        let status = if errors.is_empty() {
            if warnings.is_empty() {
                ProductionPackageItemStatus::Ready
            } else {
                ProductionPackageItemStatus::Warning
            }
        } else {
            ProductionPackageItemStatus::Blocked
        };

        ProductionPackageItemInspection {
            id: parsed.id,
            name: parsed.name,
            text: parsed.text,
            image_prompt: parsed.image_prompt,
            video_prompt_preview: prompt_preview(&parsed.video_prompt),
            video_prompt: parsed.video_prompt,
            episode: parsed.episode,
            scene: parsed.scene,
            mode,
            duration_seconds,
            width,
            height,
            first_frame,
            last_frame,
            reference_images,
            reference_audios,
            reference_videos,
            resolved_workflow_version_id: None,
            resolved_recipe_id: None,
            workflow_resolution_source: None,
            recipe_compatibility: None,
            status,
            warnings,
            errors,
        }
    }

    async fn inspect_media_list(
        &self,
        package_root: &Path,
        paths: &[String],
        kind: MediaKind,
        warnings: &mut Vec<ProductionPackageDiagnostic>,
        errors: &mut Vec<ProductionPackageDiagnostic>,
    ) -> Vec<ProductionPackageMediaInspection> {
        let mut inspected = Vec::with_capacity(paths.len());
        for path in paths {
            let (media, diagnostics) = self.inspect_media(package_root, path, kind).await;
            append_media_diagnostics(warnings, errors, diagnostics);
            inspected.push(media);
        }
        inspected
    }

    async fn inspect_media(
        &self,
        package_root: &Path,
        relative_path: &str,
        kind: MediaKind,
    ) -> (
        ProductionPackageMediaInspection,
        Vec<ProductionPackageDiagnostic>,
    ) {
        let mut media = ProductionPackageMediaInspection::empty(relative_path, kind);
        let mut diagnostics = Vec::new();
        let relative = match validate_relative_media_path(relative_path) {
            Ok(relative) => relative,
            Err(message) => {
                diagnostics.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_PATH_INVALID",
                    message,
                    None,
                ));
                return (media, diagnostics);
            }
        };
        let resolved = match canonical_media_path(package_root, &relative) {
            Ok(path) => path,
            Err(MediaPathError::Missing) => {
                diagnostics.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_MEDIA_MISSING",
                    "media file does not exist",
                    None,
                ));
                return (media, diagnostics);
            }
            Err(MediaPathError::OutsideRoot(message)) => {
                diagnostics.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_PATH_INVALID",
                    message,
                    None,
                ));
                return (media, diagnostics);
            }
            Err(
                MediaPathError::NotRegularFile(message)
                | MediaPathError::Unreadable(message)
                | MediaPathError::Invalid(message),
            ) => {
                diagnostics.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_MEDIA_INVALID",
                    message,
                    None,
                ));
                return (media, diagnostics);
            }
        };
        media.exists = true;
        media.regular_file = true;
        media.resolved_path = Some(resolved.clone());
        let metadata = match fs::metadata(&resolved) {
            Ok(metadata) => metadata,
            Err(error) => {
                diagnostics.push(ProductionPackageDiagnostic::error(
                    "PACKAGE_MEDIA_INVALID",
                    format!("media metadata is unavailable: {error}"),
                    None,
                ));
                return (media, diagnostics);
            }
        };
        media.size_bytes = Some(metadata.len());
        if metadata.len() > kind.max_bytes() {
            diagnostics.push(ProductionPackageDiagnostic::error(
                "PACKAGE_MEDIA_INVALID",
                format!("media exceeds the {} byte limit", kind.max_bytes()),
                None,
            ));
            return (media, diagnostics);
        }

        let extension = relative
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if !supported_extension(kind, &extension) {
            diagnostics.push(ProductionPackageDiagnostic::error(
                "PACKAGE_MEDIA_INVALID",
                format!("unsupported {} media format", kind.as_str()),
                None,
            ));
            return (media, diagnostics);
        }

        match kind {
            MediaKind::Image => {
                let bytes = match tokio::fs::read(&resolved).await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        diagnostics.push(ProductionPackageDiagnostic::error(
                            "PACKAGE_MEDIA_INVALID",
                            format!("media is not readable: {error}"),
                            None,
                        ));
                        return (media, diagnostics);
                    }
                };
                match crate::application::image_inspection::inspect_bytes(&bytes) {
                    Ok(inspected) => {
                        media.readable = true;
                        media.format = Some(inspected.extension.to_owned());
                        media.mime_type = Some(inspected.mime_type.to_owned());
                        media.width = Some(inspected.width);
                        media.height = Some(inspected.height);
                        media.sha256 = Some(inspected.sha256);
                    }
                    Err(error) => diagnostics.push(ProductionPackageDiagnostic::error(
                        "PACKAGE_MEDIA_INVALID",
                        format!("image validation failed: {error}"),
                        None,
                    )),
                }
            }
            MediaKind::Video => {
                let (prefix, hash) = match read_prefix_and_hash(&resolved).await {
                    Ok(value) => value,
                    Err(message) => {
                        diagnostics.push(ProductionPackageDiagnostic::error(
                            "PACKAGE_MEDIA_INVALID",
                            message,
                            None,
                        ));
                        return (media, diagnostics);
                    }
                };
                if !valid_video_signature(&extension, &prefix) {
                    diagnostics.push(ProductionPackageDiagnostic::error(
                        "PACKAGE_MEDIA_INVALID",
                        "video signature does not match a supported format",
                        None,
                    ));
                    return (media, diagnostics);
                }
                media.readable = true;
                media.format = Some(extension.clone());
                media.mime_type = Some(video_mime(&extension).to_owned());
                media.sha256 = Some(hash);
                let metadata = self.media_probe.probe_video(&resolved).await;
                media.width = metadata.width;
                media.height = metadata.height;
                media.duration_ms = metadata.duration_ms;
                if media.width.is_none() || media.height.is_none() {
                    diagnostics.push(ProductionPackageDiagnostic::warning(
                        "PACKAGE_MEDIA_DIMENSIONS_UNAVAILABLE",
                        "video dimensions could not be probed during inspection",
                        None,
                    ));
                }
            }
            MediaKind::Audio => {
                let (prefix, hash) = match read_prefix_and_hash(&resolved).await {
                    Ok(value) => value,
                    Err(message) => {
                        diagnostics.push(ProductionPackageDiagnostic::error(
                            "PACKAGE_MEDIA_INVALID",
                            message,
                            None,
                        ));
                        return (media, diagnostics);
                    }
                };
                if !valid_audio_signature(&extension, &prefix) {
                    diagnostics.push(ProductionPackageDiagnostic::error(
                        "PACKAGE_MEDIA_INVALID",
                        "audio signature does not match a supported format",
                        None,
                    ));
                    return (media, diagnostics);
                }
                media.readable = true;
                media.format = Some(extension.clone());
                media.mime_type = Some(audio_mime(&extension).to_owned());
                media.sha256 = Some(hash);
                let metadata: AudioMetadata = self.media_probe.probe_audio(&resolved).await;
                media.duration_ms = metadata.duration_ms;
                if media.duration_ms.is_none() {
                    diagnostics.push(ProductionPackageDiagnostic::warning(
                        "PACKAGE_MEDIA_DURATION_UNAVAILABLE",
                        "audio duration could not be probed during inspection",
                        None,
                    ));
                }
            }
        }
        (media, diagnostics)
    }
}

pub async fn inspect_production_package(
    package_root: impl AsRef<Path>,
) -> Result<ProductionPackageInspection, ProductionPackageInspectionError> {
    ProductionPackageInspector::default()
        .inspect(package_root)
        .await
}

pub async fn inspect_package(
    package_root: impl AsRef<Path>,
) -> Result<ProductionPackageInspection, ProductionPackageInspectionError> {
    inspect_production_package(package_root).await
}

fn canonical_directory(path: &Path) -> Result<PathBuf, ProductionPackageInspectionError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        ProductionPackageInspectionError::PackagePathInvalid {
            message: format!("package root is not readable: {error}"),
        }
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        ProductionPackageInspectionError::PackagePathInvalid {
            message: format!("package root metadata is unavailable: {error}"),
        }
    })?;
    if !metadata.is_dir() {
        return Err(ProductionPackageInspectionError::PackagePathInvalid {
            message: "package root must be a directory".to_owned(),
        });
    }
    Ok(canonical)
}

fn canonical_manifest_path(
    root: &Path,
    manifest: &Path,
) -> Result<PathBuf, ProductionPackageInspectionError> {
    let metadata = fs::symlink_metadata(manifest).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProductionPackageInspectionError::PackageJsonMissing
        } else {
            ProductionPackageInspectionError::PackageJsonUnreadable {
                message: error.to_string(),
            }
        }
    })?;
    if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        return Err(ProductionPackageInspectionError::PackageJsonInvalid {
            message: "production-package.json must be a regular file".to_owned(),
        });
    }
    let canonical = fs::canonicalize(manifest).map_err(|error| {
        ProductionPackageInspectionError::PackageJsonUnreadable {
            message: error.to_string(),
        }
    })?;
    if !canonical.starts_with(root) {
        return Err(ProductionPackageInspectionError::PackagePathInvalid {
            message: "production-package.json escapes package root".to_owned(),
        });
    }
    let canonical_metadata = fs::metadata(&canonical).map_err(|error| {
        ProductionPackageInspectionError::PackageJsonUnreadable {
            message: error.to_string(),
        }
    })?;
    if !canonical_metadata.is_file() {
        return Err(ProductionPackageInspectionError::PackageJsonInvalid {
            message: "production-package.json must be a regular file".to_owned(),
        });
    }
    Ok(canonical)
}

fn inspect_root_schema(root: &Map<String, Value>) -> Result<(), ProductionPackageInspectionError> {
    let schema_version = root.get("schemaVersion").and_then(Value::as_u64);
    if schema_version != Some(u64::from(PRODUCTION_PACKAGE_SCHEMA_VERSION)) {
        return Err(ProductionPackageInspectionError::PackageSchemaUnsupported {
            message: "schemaVersion must be 1".to_owned(),
        });
    }
    if root.get("packageType").and_then(Value::as_str) != Some(PRODUCTION_PACKAGE_TYPE) {
        return Err(ProductionPackageInspectionError::PackageTypeUnsupported {
            message: format!("packageType must be {PRODUCTION_PACKAGE_TYPE}"),
        });
    }
    Ok(())
}

fn parse_defaults(
    value: Option<&Value>,
) -> Result<ParsedDefaults, ProductionPackageInspectionError> {
    let Some(value) = value else {
        return Ok(ParsedDefaults::default());
    };
    let object = value
        .as_object()
        .ok_or_else(|| json_invalid("defaults must be an object"))?;
    let _ = unknown_fields(
        object,
        &["durationSeconds", "width", "height", "mode"],
        "defaults",
    );
    Ok(ParsedDefaults {
        duration_seconds: optional_i64(object, "durationSeconds")?,
        width: optional_i64(object, "width")?,
        height: optional_i64(object, "height")?,
        mode: optional_string_from_object(object, "mode")?,
    })
}

fn parse_item(value: &Value, index: usize) -> Result<ParsedItem, ProductionPackageInspectionError> {
    let object = value
        .as_object()
        .ok_or_else(|| json_invalid(format!("items[{index}] must be an object")))?;
    let mut errors = Vec::new();
    let id = required_item_string(object, "id", "item id", &mut errors);
    let name = required_item_string(object, "name", "item name", &mut errors);
    let video_prompt = required_item_string(object, "videoPrompt", "videoPrompt", &mut errors);
    let warnings = unknown_fields(
        object,
        &[
            "id",
            "name",
            "text",
            "imagePrompt",
            "videoPrompt",
            "episode",
            "scene",
            "durationSeconds",
            "width",
            "height",
            "mode",
            "firstFrame",
            "lastFrame",
            "referenceImages",
            "referenceAudios",
            "referenceVideos",
        ],
        &format!("items[{index}]"),
    );
    let text = optional_string_from_object(object, "text")?;
    let image_prompt = optional_string_from_object(object, "imagePrompt")?;
    let episode = optional_string_from_object(object, "episode")?;
    let scene = optional_string_from_object(object, "scene")?;
    let mode = optional_string_from_object(object, "mode")?;
    let first_frame = optional_string_from_object(object, "firstFrame")?;
    let last_frame = optional_string_from_object(object, "lastFrame")?;
    let reference_images = string_array(object, "referenceImages")?;
    let reference_audios = string_array(object, "referenceAudios")?;
    let reference_videos = string_array(object, "referenceVideos")?;

    Ok(ParsedItem {
        id,
        name,
        text,
        image_prompt,
        video_prompt,
        episode,
        scene,
        mode,
        duration_seconds: optional_i64(object, "durationSeconds")?,
        width: optional_i64(object, "width")?,
        height: optional_i64(object, "height")?,
        first_frame,
        last_frame,
        reference_images,
        reference_audios,
        reference_videos,
        warnings,
        errors,
    })
}

fn required_item_string(
    object: &Map<String, Value>,
    field: &str,
    label: &str,
    errors: &mut Vec<ProductionPackageDiagnostic>,
) -> String {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => value.clone(),
        Some(Value::String(_)) | None => {
            let code = if field == "videoPrompt" {
                "PACKAGE_PROMPT_EMPTY"
            } else {
                "PACKAGE_JSON_INVALID"
            };
            errors.push(ProductionPackageDiagnostic::error(
                code,
                format!("{label} must not be empty"),
                Some(field.to_owned()),
            ));
            String::new()
        }
        Some(_) => {
            errors.push(ProductionPackageDiagnostic::error(
                "PACKAGE_JSON_INVALID",
                format!("{label} must be a string"),
                Some(field.to_owned()),
            ));
            String::new()
        }
    }
}

fn validate_optional_metadata(
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), ProductionPackageInspectionError> {
    if let Some(value) = object.get(field) {
        if !value.is_null() && !value.is_string() && !value.is_object() && !value.is_array() {
            return Err(json_invalid(format!("{field} has an invalid JSON type")));
        }
    }
    Ok(())
}

fn unknown_fields(
    object: &Map<String, Value>,
    known: &[&str],
    prefix: &str,
) -> Vec<ProductionPackageDiagnostic> {
    let mut diagnostics = Vec::new();
    for field in object
        .keys()
        .filter(|field| !known.contains(&field.as_str()))
    {
        let field_path = if prefix.is_empty() {
            field.clone()
        } else {
            format!("{prefix}.{field}")
        };
        let code = if PACKAGE_EXECUTION_FIELDS.contains(&field.as_str()) {
            "PACKAGE_EXECUTION_FIELD_IGNORED"
        } else {
            "PACKAGE_UNKNOWN_FIELD"
        };
        let message = if code == "PACKAGE_EXECUTION_FIELD_IGNORED" {
            "execution-like field is ignored and cannot control internal production"
        } else {
            "unknown package field is preserved as metadata only"
        };
        diagnostics.push(ProductionPackageDiagnostic::warning(
            code,
            message,
            Some(field_path),
        ));
    }
    diagnostics
}

fn required_string(
    object: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String, ProductionPackageInspectionError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| json_invalid(format!("{label} must be a string")))
}

fn required_non_empty_string(
    object: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String, ProductionPackageInspectionError> {
    let value = required_string(object, field, label)?;
    if value.trim().is_empty() {
        return Err(json_invalid(format!("{label} must not be empty")));
    }
    Ok(value)
}

fn optional_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ProductionPackageInspectionError> {
    optional_string_from_object(object, field)
}

fn optional_string_from_object(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ProductionPackageInspectionError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(json_invalid(format!("{field} must be a string or null"))),
    }
}

fn optional_i64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<i64>, ProductionPackageInspectionError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| json_invalid(format!("{field} must be an integer"))),
        Some(_) => Err(json_invalid(format!("{field} must be an integer or null"))),
    }
}

fn string_array(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, ProductionPackageInspectionError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(json_invalid(format!("{field} must be an array")));
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                json_invalid(format!("{field}[{index}] must be a relative path string"))
            })
        })
        .collect()
}

fn effective_defaults(defaults: &ParsedDefaults) -> ProductionPackageDefaultsInspection {
    ProductionPackageDefaultsInspection {
        duration_seconds: defaults
            .duration_seconds
            .unwrap_or(DEFAULT_PACKAGE_DURATION_SECONDS),
        width: defaults.width.unwrap_or(DEFAULT_PACKAGE_WIDTH),
        height: defaults.height.unwrap_or(DEFAULT_PACKAGE_HEIGHT),
        mode: defaults
            .mode
            .as_deref()
            .and_then(canonical_mode)
            .map(ToOwned::to_owned),
    }
}

fn canonical_mode(value: &str) -> Option<&'static str> {
    let normalized = value
        .trim()
        .to_ascii_uppercase()
        .replace(['-', ' ', '/'], "_");
    match normalized.as_str() {
        "I2V" | "IMAGE_TO_VIDEO" | "FL2VA_IMAGE_TO_VIDEO" => Some("FL2VA_IMAGE_TO_VIDEO"),
        "FIRST_LAST" | "FIRST_LAST_FRAME" | "FIRSTLAST" | "FL2VA_FIRST_LAST" => {
            Some("FL2VA_FIRST_LAST")
        }
        "TEXT" | "TEXT_ONLY" | "T2V" | "TEXT_TO_VIDEO" | "FL2VA_TEXT_TO_VIDEO" => {
            Some("FL2VA_TEXT_TO_VIDEO")
        }
        "IMAGE" | "REFERENCE_IMAGE" | "REFERENCE_IMAGES" | "REF_IMAGE" | "REF2VA_IMAGE" => {
            Some("REF2VA_IMAGE")
        }
        "REF2VA_AUDIO" => Some("REF2VA_AUDIO"),
        "REF2VA_IMAGE_AUDIO" => Some("REF2VA_IMAGE_AUDIO"),
        "REF2VA_VIDEO_IMAGE" => Some("REF2VA_VIDEO_IMAGE"),
        _ => None,
    }
}

fn infer_mode(item: &ParsedItem) -> Option<&'static str> {
    if item.first_frame.is_some() && item.last_frame.is_some() {
        Some("FL2VA_FIRST_LAST")
    } else if item.first_frame.is_some() {
        Some("FL2VA_IMAGE_TO_VIDEO")
    } else if !item.reference_images.is_empty() && !item.reference_audios.is_empty() {
        Some("REF2VA_IMAGE_AUDIO")
    } else if !item.reference_images.is_empty() && !item.reference_videos.is_empty() {
        Some("REF2VA_VIDEO_IMAGE")
    } else if !item.reference_audios.is_empty() {
        Some("REF2VA_AUDIO")
    } else if !item.reference_images.is_empty() {
        Some("REF2VA_IMAGE")
    } else {
        Some("FL2VA_TEXT_TO_VIDEO")
    }
}

fn validate_mode_inputs(
    mode: &str,
    has_first: bool,
    has_last: bool,
    has_images: bool,
    has_audios: bool,
    has_videos: bool,
    errors: &mut Vec<ProductionPackageDiagnostic>,
) {
    let missing = |message: &str, field: &str, errors: &mut Vec<ProductionPackageDiagnostic>| {
        errors.push(ProductionPackageDiagnostic::error(
            "PACKAGE_MEDIA_INVALID",
            message,
            Some(field.to_owned()),
        ));
    };
    match mode {
        "FL2VA_IMAGE_TO_VIDEO" if !has_first => missing(
            "image-to-video mode requires firstFrame",
            "firstFrame",
            errors,
        ),
        "FL2VA_FIRST_LAST" if !has_first || !has_last => missing(
            "first-last mode requires firstFrame and lastFrame",
            "firstFrame/lastFrame",
            errors,
        ),
        "FL2VA_FIRST_LAST" if has_first && has_last => {}
        "REF2VA_IMAGE" if !has_images => missing(
            "reference-image mode requires referenceImages",
            "referenceImages",
            errors,
        ),
        "REF2VA_AUDIO" if !has_audios => missing(
            "reference-audio mode requires referenceAudios",
            "referenceAudios",
            errors,
        ),
        "REF2VA_IMAGE_AUDIO" if !has_images || !has_audios => missing(
            "image-audio mode requires referenceImages and referenceAudios",
            "referenceImages/referenceAudios",
            errors,
        ),
        "REF2VA_VIDEO_IMAGE" if !has_images || !has_videos => missing(
            "video-image mode requires referenceImages and referenceVideos",
            "referenceImages/referenceVideos",
            errors,
        ),
        _ => {}
    }
}

fn warn_about_unused_media(
    mode: &str,
    has_first: bool,
    has_last: bool,
    has_images: bool,
    has_audios: bool,
    has_videos: bool,
    warnings: &mut Vec<ProductionPackageDiagnostic>,
) {
    let uses_first = matches!(mode, "FL2VA_IMAGE_TO_VIDEO" | "FL2VA_FIRST_LAST");
    let uses_last = mode == "FL2VA_FIRST_LAST";
    let uses_images = matches!(
        mode,
        "REF2VA_IMAGE" | "REF2VA_IMAGE_AUDIO" | "REF2VA_VIDEO_IMAGE"
    );
    let uses_audios = matches!(mode, "REF2VA_AUDIO" | "REF2VA_IMAGE_AUDIO");
    let uses_videos = mode == "REF2VA_VIDEO_IMAGE";
    if has_first && !uses_first {
        warnings.push(ProductionPackageDiagnostic::warning(
            "PACKAGE_MEDIA_UNUSED",
            "firstFrame is not consumed by the selected mode",
            Some("firstFrame".to_owned()),
        ));
    }
    if has_last && !uses_last {
        warnings.push(ProductionPackageDiagnostic::warning(
            "PACKAGE_MEDIA_UNUSED",
            "lastFrame is not consumed by the selected mode",
            Some("lastFrame".to_owned()),
        ));
    }
    if has_images && !uses_images && !uses_first {
        warnings.push(ProductionPackageDiagnostic::warning(
            "PACKAGE_MEDIA_UNUSED",
            "referenceImages are not consumed by the selected mode",
            Some("referenceImages".to_owned()),
        ));
    }
    if has_audios && !uses_audios {
        warnings.push(ProductionPackageDiagnostic::warning(
            "PACKAGE_MEDIA_UNUSED",
            "referenceAudios are not consumed by the selected mode",
            Some("referenceAudios".to_owned()),
        ));
    }
    if has_videos && !uses_videos {
        warnings.push(ProductionPackageDiagnostic::warning(
            "PACKAGE_MEDIA_UNUSED",
            "referenceVideos are not consumed by the selected mode",
            Some("referenceVideos".to_owned()),
        ));
    }
}

fn append_media_diagnostics(
    warnings: &mut Vec<ProductionPackageDiagnostic>,
    errors: &mut Vec<ProductionPackageDiagnostic>,
    diagnostics: Vec<ProductionPackageDiagnostic>,
) {
    for diagnostic in diagnostics {
        match diagnostic.severity {
            ProductionPackageDiagnosticSeverity::Warning => warnings.push(diagnostic),
            ProductionPackageDiagnosticSeverity::Error => errors.push(diagnostic),
        }
    }
}

fn prompt_preview(prompt: &str) -> String {
    prompt
        .chars()
        .take(MAX_PACKAGE_PROMPT_PREVIEW_CHARS)
        .collect()
}

fn validate_relative_media_path(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    crate::domain::production_package::validate_package_relative_path(trimmed)
        .map_err(|error| error.to_string())?;
    let path = PathBuf::from(trimmed);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("media path must be relative to the package root".to_owned());
    }
    Ok(path)
}

#[derive(Debug)]
enum MediaPathError {
    Missing,
    OutsideRoot(String),
    Invalid(String),
    NotRegularFile(String),
    Unreadable(String),
}

fn canonical_media_path(root: &Path, relative: &Path) -> Result<PathBuf, MediaPathError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(MediaPathError::Invalid(
                "media path contains a non-normal component".to_owned(),
            ));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MediaPathError::Missing
            } else {
                MediaPathError::Unreadable(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink() {
            let canonical = fs::canonicalize(&current)
                .map_err(|error| MediaPathError::Unreadable(error.to_string()))?;
            if !canonical.starts_with(root) {
                return Err(MediaPathError::OutsideRoot(
                    "media path escapes the package root through a symlink".to_owned(),
                ));
            }
            return Err(MediaPathError::Invalid(
                "media path cannot traverse a symlink".to_owned(),
            ));
        }
        let canonical = fs::canonicalize(&current)
            .map_err(|error| MediaPathError::Unreadable(error.to_string()))?;
        if !canonical.starts_with(root) {
            return Err(MediaPathError::OutsideRoot(
                "media path escapes the package root".to_owned(),
            ));
        }
    }
    let metadata =
        fs::metadata(&current).map_err(|error| MediaPathError::Unreadable(error.to_string()))?;
    if !metadata.is_file() {
        return Err(MediaPathError::NotRegularFile(
            "media path must point to a regular file".to_owned(),
        ));
    }
    Ok(current)
}

fn supported_extension(kind: MediaKind, extension: &str) -> bool {
    match kind {
        MediaKind::Image => matches!(extension, "png" | "jpg" | "jpeg" | "webp"),
        MediaKind::Video => matches!(extension, "mp4" | "webm" | "mov" | "mkv"),
        MediaKind::Audio => matches!(extension, "wav" | "flac" | "mp3" | "ogg" | "opus" | "m4a"),
    }
}

fn valid_video_signature(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "mp4" | "mov" => is_ftyp(bytes),
        "webm" | "mkv" => is_ebml(bytes),
        _ => false,
    }
}

fn valid_audio_signature(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "wav" => is_wav(bytes),
        "flac" => bytes.starts_with(b"fLaC"),
        "mp3" => is_mp3(bytes),
        "ogg" | "opus" => is_ogg(bytes),
        "m4a" => is_ftyp(bytes),
        _ => false,
    }
}

fn is_ftyp(bytes: &[u8]) -> bool {
    bytes.get(4..8) == Some(b"ftyp")
}

fn is_ebml(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
}

fn is_wav(bytes: &[u8]) -> bool {
    bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WAVE")
}

fn is_mp3(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xff && (bytes[1] & 0xe0) == 0xe0)
}

fn is_ogg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
}

fn video_mime(extension: &str) -> &'static str {
    match extension {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

fn audio_mime(extension: &str) -> &'static str {
    match extension {
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    }
}

async fn read_prefix_and_hash(path: &Path) -> Result<(Vec<u8>, String), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("media is not readable: {error}"))?;
    let mut hasher = Sha256::new();
    let mut prefix = Vec::with_capacity(64);
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("media is not readable: {error}"))?;
        if count == 0 {
            break;
        }
        if prefix.len() < 64 {
            prefix.extend_from_slice(&buffer[..count.min(64 - prefix.len())]);
        }
        hasher.update(&buffer[..count]);
    }
    Ok((prefix, format!("{:x}", hasher.finalize())))
}

async fn hash_file(path: &Path) -> Result<String, String> {
    read_prefix_and_hash(path).await.map(|(_, hash)| hash)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json_invalid(message: impl Into<String>) -> ProductionPackageInspectionError {
    ProductionPackageInspectionError::PackageJsonInvalid {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use image::{DynamicImage, ImageFormat};
    use std::{io::Cursor, path::Path};
    use tempfile::tempdir;

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(1, 1)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("png fixture should encode");
        bytes.into_inner()
    }

    fn package_json(items: Value) -> Value {
        serde_json::json!({
            "schemaVersion": 1,
            "packageType": PRODUCTION_PACKAGE_TYPE,
            "name": "EP01",
            "defaults": {"durationSeconds": 5, "width": 864, "height": 480},
            "items": items
        })
    }

    fn item(id: &str, prompt: &str) -> Value {
        serde_json::json!({
            "id": id,
            "name": id,
            "videoPrompt": prompt,
            "mode": "TEXT_ONLY"
        })
    }

    async fn write_package(root: &Path, document: &Value) {
        tokio::fs::write(
            root.join("production-package.json"),
            serde_json::to_vec(document).expect("package should serialize"),
        )
        .await
        .expect("manifest should write");
    }

    #[tokio::test]
    async fn inspects_valid_i2v_media_without_writing() {
        let root = tempdir().expect("temp root");
        tokio::fs::create_dir(root.path().join("images"))
            .await
            .expect("images directory");
        let image = png_bytes();
        tokio::fs::write(root.path().join("images/SH001.png"), &image)
            .await
            .expect("image fixture");
        write_package(
            root.path(),
            &package_json(serde_json::json!([{
                "id": "EP01-SH001",
                "name": "镜头001",
                "videoPrompt": "camera slowly pushes in",
                "firstFrame": "images/SH001.png",
                "mode": "I2V"
            }])),
        )
        .await;

        let before = std::fs::read_dir(root.path())
            .expect("root listing")
            .count();
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("valid package should inspect");
        let after = std::fs::read_dir(root.path())
            .expect("root listing")
            .count();
        assert_eq!(before, after);
        assert_eq!(inspection.item_count, 1);
        assert_eq!(inspection.ready_count, 1);
        assert_eq!(inspection.blocked_count, 0);
        assert_eq!(
            inspection.items[0].status,
            ProductionPackageItemStatus::Ready
        );
        assert_eq!(inspection.items[0].mode, "FL2VA_IMAGE_TO_VIDEO");
        let frame = inspection.items[0]
            .first_frame
            .as_ref()
            .expect("first frame metadata");
        assert!(frame.readable);
        assert_eq!(frame.width, Some(1));
        assert_eq!(frame.height, Some(1));
        assert_eq!(frame.sha256.as_deref(), Some(sha256(&image).as_str()));
    }

    #[tokio::test]
    async fn rejects_schema_empty_and_duplicate_packages_with_stable_codes() {
        let root = tempdir().expect("temp root");
        write_package(
            root.path(),
            &serde_json::json!({
                "schemaVersion": 2,
                "packageType": PRODUCTION_PACKAGE_TYPE,
                "name": "bad",
                "items": [item("one", "prompt")]
            }),
        )
        .await;
        let error = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect_err("wrong schema should fail closed");
        assert_eq!(error.code(), "PACKAGE_SCHEMA_UNSUPPORTED");

        write_package(root.path(), &package_json(serde_json::json!([]))).await;
        let error = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect_err("empty package should fail closed");
        assert_eq!(error.code(), "PACKAGE_EMPTY");

        write_package(
            root.path(),
            &package_json(serde_json::json!([
                item("duplicate", "one"),
                item("duplicate", "two")
            ])),
        )
        .await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("duplicate ids are item blockers");
        assert_eq!(inspection.blocked_count, 2);
        assert!(inspection.items.iter().all(|item| item
            .errors
            .iter()
            .any(|error| error.code == "PACKAGE_DUPLICATE_ITEM_ID")));
    }

    #[tokio::test]
    async fn unknown_and_execution_like_fields_are_warning_only_and_inert() {
        let root = tempdir().expect("temp root");
        write_package(
            root.path(),
            &serde_json::json!({
                "schemaVersion": 1,
                "packageType": PRODUCTION_PACKAGE_TYPE,
                "name": "EP01",
                "workflowVersionId": "do-not-use",
                "defaults": {"mode": "TEXT_ONLY", "unknownDefault": true},
                "items": [{
                    "id": "one",
                    "name": "one",
                    "videoPrompt": "safe prompt",
                    "taskId": "do-not-use",
                    "unknown": {"selectedVideoAssetId": "do-not-use"}
                }]
            }),
        )
        .await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("unknown fields should not block");
        assert_eq!(inspection.items[0].mode, "FL2VA_TEXT_TO_VIDEO");
        assert_eq!(
            inspection.items[0].status,
            ProductionPackageItemStatus::Warning
        );
        assert!(inspection
            .warnings
            .iter()
            .any(|warning| warning.code == "PACKAGE_EXECUTION_FIELD_IGNORED"));
        assert!(inspection.items[0]
            .warnings
            .iter()
            .any(|warning| warning.code == "PACKAGE_EXECUTION_FIELD_IGNORED"));
    }

    #[tokio::test]
    async fn missing_item_name_is_a_blocked_item_not_a_silent_drop() {
        let root = tempdir().expect("temp root");
        write_package(
            root.path(),
            &package_json(serde_json::json!([{
                "id": "one",
                "videoPrompt": "prompt",
                "mode": "TEXT_ONLY"
            }])),
        )
        .await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("item validation should remain inspectable");
        assert_eq!(inspection.blocked_count, 1);
        assert!(inspection.items[0]
            .errors
            .iter()
            .any(|error| error.code == "PACKAGE_JSON_INVALID"));
    }

    #[tokio::test]
    async fn blocks_escape_missing_invalid_and_non_file_media() {
        let root = tempdir().expect("temp root");
        tokio::fs::create_dir(root.path().join("images"))
            .await
            .expect("images directory");
        tokio::fs::write(root.path().join("images/not-image.png"), b"not png")
            .await
            .expect("invalid image");
        write_package(
            root.path(),
            &package_json(serde_json::json!([{
                "id": "escape",
                "name": "escape",
                "videoPrompt": "prompt",
                "firstFrame": "../outside.png"
            }, {
                "id": "absolute",
                "name": "absolute",
                "videoPrompt": "prompt",
                "firstFrame": "C:/outside.png"
            }, {
                "id": "missing",
                "name": "missing",
                "videoPrompt": "prompt",
                "firstFrame": "images/missing.png"
            }, {
                "id": "invalid",
                "name": "invalid",
                "videoPrompt": "prompt",
                "firstFrame": "images/not-image.png"
            }, {
                "id": "directory",
                "name": "directory",
                "videoPrompt": "prompt",
                "firstFrame": "images"
            }])),
        )
        .await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("item media failures belong in inspection");
        assert_eq!(inspection.blocked_count, 5);
        assert_eq!(inspection.items[0].errors[0].code, "PACKAGE_PATH_INVALID");
        assert_eq!(inspection.items[1].errors[0].code, "PACKAGE_PATH_INVALID");
        assert_eq!(inspection.items[2].errors[0].code, "PACKAGE_MEDIA_MISSING");
        assert_eq!(inspection.items[3].errors[0].code, "PACKAGE_MEDIA_INVALID");
        assert_eq!(inspection.items[4].errors[0].code, "PACKAGE_MEDIA_INVALID");
    }

    #[tokio::test]
    async fn prompt_preview_is_300_unicode_scalars_and_large_prompt_is_blocked() {
        let root = tempdir().expect("temp root");
        let long_prompt = "界".repeat(MAX_PACKAGE_PROMPT_BYTES);
        write_package(
            root.path(),
            &package_json(serde_json::json!([{
                "id": "long",
                "name": "long",
                "videoPrompt": long_prompt,
                "mode": "TEXT_ONLY"
            }])),
        )
        .await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("large prompt is an item error");
        assert_eq!(inspection.blocked_count, 1);
        assert_eq!(
            inspection.items[0].video_prompt_preview.chars().count(),
            300
        );
        assert!(inspection.items[0]
            .errors
            .iter()
            .any(|error| error.code == "PACKAGE_PROMPT_TOO_LARGE"));
    }

    #[tokio::test]
    async fn accepts_exactly_500_items_and_rejects_501() {
        let root = tempdir().expect("temp root");
        let items = (0..MAX_PRODUCTION_PACKAGE_ITEMS)
            .map(|index| item(&format!("item-{index}"), "prompt"))
            .collect::<Vec<_>>();
        write_package(root.path(), &package_json(Value::Array(items))).await;
        let inspection = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect("500 items should be accepted");
        assert_eq!(inspection.item_count, MAX_PRODUCTION_PACKAGE_ITEMS);
        assert_eq!(inspection.ready_count, MAX_PRODUCTION_PACKAGE_ITEMS);

        let items = (0..=MAX_PRODUCTION_PACKAGE_ITEMS)
            .map(|index| item(&format!("item-{index}"), "prompt"))
            .collect::<Vec<_>>();
        write_package(root.path(), &package_json(Value::Array(items))).await;
        let error = ProductionPackageInspector::default()
            .inspect(root.path())
            .await
            .expect_err("501 items should fail closed");
        assert_eq!(error.code(), "PACKAGE_TOO_LARGE");
    }

    #[tokio::test]
    async fn revalidation_detects_media_sha_change() {
        let root = tempdir().expect("temp root");
        tokio::fs::create_dir(root.path().join("images"))
            .await
            .expect("images directory");
        tokio::fs::write(root.path().join("images/SH001.png"), png_bytes())
            .await
            .expect("image fixture");
        write_package(
            root.path(),
            &package_json(serde_json::json!([{
                "id": "one",
                "name": "one",
                "videoPrompt": "prompt",
                "firstFrame": "images/SH001.png",
                "mode": "I2V"
            }])),
        )
        .await;
        let inspector = ProductionPackageInspector::default();
        let inspection = inspector.inspect(root.path()).await.expect("inspection");
        let media = inspection.items[0].first_frame.as_ref().expect("frame");
        tokio::fs::write(root.path().join("images/SH001.png"), b"changed")
            .await
            .expect("changed media");
        let error = inspector
            .revalidate_media(root.path(), media)
            .await
            .expect_err("changed media should be rejected");
        assert_eq!(error.code(), "PACKAGE_MEDIA_CHANGED");
    }

    #[test]
    fn relative_path_validation_rejects_windows_and_url_forms() {
        for path in [
            "../outside.png",
            "..\\outside.png",
            "C:/outside.png",
            "C:\\outside.png",
            "\\\\server\\share\\outside.png",
            "/tmp/outside.png",
            "~/outside.png",
            "https://example.test/image.png",
        ] {
            assert!(validate_relative_media_path(path).is_err(), "{path}");
        }
        assert!(validate_relative_media_path("images/SH001.png").is_ok());
    }

    #[allow(dead_code)]
    struct FakeMediaProbe;

    #[async_trait]
    impl MediaProbe for FakeMediaProbe {
        async fn probe_video(
            &self,
            _path: &Path,
        ) -> crate::application::media_probe::VideoMetadata {
            crate::application::media_probe::VideoMetadata {
                width: Some(864),
                height: Some(480),
                duration_ms: Some(5_000),
            }
        }

        async fn generate_video_poster(&self, _path: &Path) -> Option<Vec<u8>> {
            None
        }
    }
}
