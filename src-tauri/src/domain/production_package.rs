//! Domain contract for the external Production Package V1 format.
//!
//! A production package is an input document, not a persisted database
//! entity. `package_id` and item `id` deliberately remain opaque external
//! labels; they must never be parsed as `ProductionBatchId`, `AssetId`, or any
//! other formal database identifier.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, path::Path};

pub const PRODUCTION_PACKAGE_SCHEMA_VERSION: u32 = 1;
pub const PRODUCTION_PACKAGE_TYPE: &str = "AI_STUDIO_VIDEO_PRODUCTION";
pub const PRODUCTION_PACKAGE_MAX_ITEMS: usize = 500;
pub const PRODUCTION_PACKAGE_PROMPT_PREVIEW_CHARS: usize = 300;

pub const CODE_PACKAGE_JSON_INVALID: &str = "PACKAGE_JSON_INVALID";
pub const CODE_PACKAGE_SCHEMA_UNSUPPORTED: &str = "PACKAGE_SCHEMA_UNSUPPORTED";
pub const CODE_PACKAGE_EMPTY: &str = "PACKAGE_EMPTY";
pub const CODE_PACKAGE_TOO_LARGE: &str = "PACKAGE_TOO_LARGE";
pub const CODE_PACKAGE_DUPLICATE_ITEM_ID: &str = "PACKAGE_DUPLICATE_ITEM_ID";
pub const CODE_PACKAGE_PATH_INVALID: &str = "PACKAGE_PATH_INVALID";
pub const CODE_PACKAGE_MEDIA_MISSING: &str = "PACKAGE_MEDIA_MISSING";
pub const CODE_PACKAGE_MEDIA_INVALID: &str = "PACKAGE_MEDIA_INVALID";
pub const CODE_PACKAGE_MEDIA_CHANGED: &str = "PACKAGE_MEDIA_CHANGED";
pub const CODE_PACKAGE_UNKNOWN_FIELD: &str = "PACKAGE_UNKNOWN_FIELD";
pub const CODE_PACKAGE_PROMPT_EMPTY: &str = "PACKAGE_PROMPT_EMPTY";
pub const CODE_PACKAGE_PROMPT_TOO_LARGE: &str = "PACKAGE_PROMPT_TOO_LARGE";
pub const CODE_PACKAGE_MODE_UNSUPPORTED: &str = "PACKAGE_MODE_UNSUPPORTED";
pub const CODE_PACKAGE_RESOLUTION_UNSUPPORTED: &str = "PACKAGE_RESOLUTION_UNSUPPORTED";
pub const CODE_PACKAGE_DURATION_INVALID: &str = "PACKAGE_DURATION_INVALID";
pub const CODE_PACKAGE_TYPE_UNSUPPORTED: &str = "PACKAGE_TYPE_UNSUPPORTED";
pub const CODE_PACKAGE_EXECUTION_FIELD_IGNORED: &str = "PACKAGE_EXECUTION_FIELD_IGNORED";

/// These fields are data-only labels. They are never trusted as workflow,
/// queue, task, or asset identifiers.
pub const PACKAGE_EXECUTION_FIELDS: &[&str] = &[
    "workflowVersionId",
    "recipeId",
    "taskId",
    "batchId",
    "assetId",
    "comfyPromptId",
    "selectedVideoAssetId",
];

/// Canonical H3 strings already used by the local import and generation
/// paths. This is intentionally a string list, not a second H3 mode enum.
pub const SUPPORTED_H3_GENERATION_MODES: &[&str] = &[
    "FL2VA_TEXT_TO_VIDEO",
    "FL2VA_IMAGE_TO_VIDEO",
    "FL2VA_FIRST_LAST",
    "REF2VA_IMAGE",
    "REF2VA_AUDIO",
    "REF2VA_IMAGE_AUDIO",
    "REF2VA_VIDEO_IMAGE",
];

pub const PACKAGE_FIELDS: &[&str] = &[
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
];

pub const PACKAGE_DEFAULT_FIELDS: &[&str] = &["durationSeconds", "width", "height", "mode"];

pub const PACKAGE_ITEM_FIELDS: &[&str] = &[
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
];

pub const fn is_supported_package_schema(version: u32) -> bool {
    version == PRODUCTION_PACKAGE_SCHEMA_VERSION
}

pub fn is_supported_package_type(value: &str) -> bool {
    value == PRODUCTION_PACKAGE_TYPE
}

pub fn is_supported_h3_generation_mode(value: &str) -> bool {
    SUPPORTED_H3_GENERATION_MODES.contains(&value)
}

pub fn is_execution_field(value: &str) -> bool {
    PACKAGE_EXECUTION_FIELDS.contains(&value)
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDefaults {
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub mode: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackage {
    pub schema_version: u32,
    pub package_type: String,
    /// Opaque upstream label. It is not a database ID.
    pub package_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<String>,
    /// Kept as an opaque wire value so inspection remains deterministic and
    /// does not silently reinterpret provider metadata.
    pub created_at: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub defaults: ProductionPackageDefaults,
    pub items: Vec<ProductionPackageItem>,
}

impl ProductionPackage {
    pub fn new(name: impl Into<String>, items: Vec<ProductionPackageItem>) -> Self {
        Self {
            schema_version: PRODUCTION_PACKAGE_SCHEMA_VERSION,
            package_type: PRODUCTION_PACKAGE_TYPE.to_owned(),
            package_id: None,
            name: name.into(),
            description: None,
            created_by: None,
            created_at: None,
            source: None,
            defaults: ProductionPackageDefaults::default(),
            items,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageItem {
    /// Opaque upstream label. It is not a Shot, Asset, Task, or Batch ID.
    pub id: String,
    pub name: String,
    pub text: Option<String>,
    pub image_prompt: Option<String>,
    pub video_prompt: String,
    pub episode: Option<String>,
    pub scene: Option<String>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Canonical H3 mode string or an external alias to be resolved by the
    /// application layer. This domain contract does not invent a new mode.
    pub mode: Option<String>,
    pub first_frame: Option<String>,
    pub last_frame: Option<String>,
    #[serde(default)]
    pub reference_images: Vec<String>,
    #[serde(default)]
    pub reference_audios: Vec<String>,
    #[serde(default)]
    pub reference_videos: Vec<String>,
}

impl ProductionPackageItem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        video_prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            text: None,
            image_prompt: None,
            video_prompt: video_prompt.into(),
            episode: None,
            scene: None,
            duration_seconds: None,
            width: None,
            height: None,
            mode: None,
            first_frame: None,
            last_frame: None,
            reference_images: Vec::new(),
            reference_audios: Vec::new(),
            reference_videos: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductionPackageMediaKind {
    Image,
    Video,
    Audio,
    #[default]
    Unknown,
}

impl ProductionPackageMediaKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "image" => Self::Image,
            "video" => Self::Video,
            "audio" => Self::Audio,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageMediaMetadata {
    /// Always package-root-relative. The inspector is responsible for the
    /// canonical filesystem and symlink/junction boundary checks.
    pub relative_path: String,
    pub kind: ProductionPackageMediaKind,
    pub exists: bool,
    pub is_file: bool,
    pub readable: bool,
    pub format_supported: bool,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<u64>,
}

impl ProductionPackageMediaMetadata {
    pub fn new(relative_path: impl Into<String>, kind: ProductionPackageMediaKind) -> Self {
        Self {
            relative_path: relative_path.into(),
            kind,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductionPackageItemStatus {
    Ready,
    Warning,
    Blocked,
}

impl Default for ProductionPackageItemStatus {
    fn default() -> Self {
        Self::Blocked
    }
}

impl ProductionPackageItemStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Warning => "WARNING",
            Self::Blocked => "BLOCKED",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "READY" => Self::Ready,
            "WARNING" => Self::Warning,
            "BLOCKED" => Self::Blocked,
            _ => return None,
        })
    }

    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub const fn is_blocked(self) -> bool {
        matches!(self, Self::Blocked)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductionPackageDiagnosticSeverity {
    #[default]
    Info,
    Warning,
    Error,
    Blocker,
}

impl ProductionPackageDiagnosticSeverity {
    pub const fn blocks(self) -> bool {
        matches!(self, Self::Error | Self::Blocker)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageDiagnostic {
    pub severity: ProductionPackageDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

impl ProductionPackageDiagnostic {
    pub fn new(
        severity: ProductionPackageDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            field: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductionPackageDiagnosticSeverity::Warning, code, message)
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductionPackageDiagnosticSeverity::Error, code, message)
    }

    pub fn blocker(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductionPackageDiagnosticSeverity::Blocker, code, message)
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductionPackageInspectionStatus {
    Ready,
    Warning,
    Blocked,
}

impl Default for ProductionPackageInspectionStatus {
    fn default() -> Self {
        Self::Blocked
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageInspectionItem {
    /// The same opaque external label supplied by the package.
    pub id: String,
    pub name: String,
    pub mode: Option<String>,
    pub video_prompt_preview: Option<String>,
    pub first_frame: Option<ProductionPackageMediaMetadata>,
    pub last_frame: Option<ProductionPackageMediaMetadata>,
    pub references: Vec<ProductionPackageMediaMetadata>,
    pub duration_seconds: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub status: ProductionPackageItemStatus,
    pub warnings: Vec<ProductionPackageDiagnostic>,
    pub errors: Vec<ProductionPackageDiagnostic>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionPackageInspection {
    pub package_name: String,
    pub package_id: Option<String>,
    pub item_count: usize,
    pub ready_count: usize,
    pub warning_count: usize,
    pub blocked_count: usize,
    pub status: ProductionPackageInspectionStatus,
    pub items: Vec<ProductionPackageInspectionItem>,
    pub warnings: Vec<ProductionPackageDiagnostic>,
    pub errors: Vec<ProductionPackageDiagnostic>,
}

impl ProductionPackageInspection {
    pub fn from_items(
        package_name: impl Into<String>,
        package_id: Option<String>,
        items: Vec<ProductionPackageInspectionItem>,
        warnings: Vec<ProductionPackageDiagnostic>,
        errors: Vec<ProductionPackageDiagnostic>,
    ) -> Self {
        let ready_count = items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Ready)
            .count();
        let warning_count = items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Warning)
            .count();
        let blocked_count = items
            .iter()
            .filter(|item| item.status == ProductionPackageItemStatus::Blocked)
            .count();
        let status = if items.is_empty()
            || blocked_count > 0
            || errors.iter().any(|item| item.severity.blocks())
        {
            ProductionPackageInspectionStatus::Blocked
        } else if warning_count > 0 || !warnings.is_empty() {
            ProductionPackageInspectionStatus::Warning
        } else {
            ProductionPackageInspectionStatus::Ready
        };

        Self {
            package_name: package_name.into(),
            package_id,
            item_count: items.len(),
            ready_count,
            warning_count,
            blocked_count,
            status,
            items,
            warnings,
            errors,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionPackagePathError {
    Empty,
    Absolute,
    ParentTraversal,
    Url,
    Invalid,
}

impl ProductionPackagePathError {
    pub const fn code(&self) -> &'static str {
        CODE_PACKAGE_PATH_INVALID
    }
}

impl fmt::Display for ProductionPackagePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "package media path must not be empty",
            Self::Absolute => "package media path must be relative",
            Self::ParentTraversal => "package media path must not contain parent traversal",
            Self::Url => "package media path must not be a URL",
            Self::Invalid => "package media path is invalid",
        };
        write!(formatter, "{}: {message}", self.code())
    }
}

impl Error for ProductionPackagePathError {}

pub fn is_safe_package_relative_path(value: &str) -> bool {
    validate_package_relative_path(value).is_ok()
}

pub fn validate_package_relative_path(value: &str) -> Result<(), ProductionPackagePathError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ProductionPackagePathError::Empty);
    }
    if trimmed.contains('\0') {
        return Err(ProductionPackagePathError::Invalid);
    }
    if trimmed.contains("://") {
        return Err(ProductionPackagePathError::Url);
    }
    if trimmed == "~" || trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        return Err(ProductionPackagePathError::Absolute);
    }

    let path = Path::new(trimmed);
    if path.is_absolute()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || is_windows_drive_path(trimmed)
    {
        return Err(ProductionPackagePathError::Absolute);
    }
    if trimmed
        .split(['/', '\\'])
        .any(|component| component == "..")
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ProductionPackagePathError::ParentTraversal);
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::RootDir | std::path::Component::Prefix(_)
        )
    }) {
        return Err(ProductionPackagePathError::Absolute);
    }
    Ok(())
}

fn is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn package_wire_uses_camel_case_and_preserves_external_labels() {
        let mut item =
            ProductionPackageItem::new("EP01-SC01-SH001", "镜头001", "camera slowly pushes in");
        item.first_frame = Some("images/SH001.png".to_owned());
        item.reference_images = vec!["references/hero.png".to_owned()];
        item.duration_seconds = Some(5);
        item.width = Some(864);
        item.height = Some(480);

        let mut package = ProductionPackage::new("EP01", vec![item]);
        package.package_id = Some("kṣitigarbha-ep01-v1".to_owned());
        package.defaults = ProductionPackageDefaults {
            duration_seconds: Some(5),
            width: Some(864),
            height: Some(480),
            mode: Some("FL2VA_IMAGE_TO_VIDEO".to_owned()),
        };

        let value = to_value(&package).expect("package should serialize");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["packageType"], PRODUCTION_PACKAGE_TYPE);
        assert_eq!(value["packageId"], "kṣitigarbha-ep01-v1");
        assert_eq!(value["items"][0]["videoPrompt"], "camera slowly pushes in");
        assert_eq!(value["items"][0]["firstFrame"], "images/SH001.png");
        assert_eq!(
            value["items"][0]["referenceImages"][0],
            "references/hero.png"
        );
        assert_eq!(value["items"][0]["durationSeconds"], 5);
        assert!(value["items"][0].get("video_prompt").is_none());

        let decoded: ProductionPackage = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.package_id.as_deref(), Some("kṣitigarbha-ep01-v1"));
        assert_eq!(decoded.items[0].id, "EP01-SC01-SH001");
    }

    #[test]
    fn missing_optional_defaults_and_media_arrays_are_safe() {
        let package: ProductionPackage = serde_json::from_value(json!({
            "schemaVersion": 1,
            "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
            "name": "EP01",
            "items": [{
                "id": "external-1",
                "name": "shot",
                "videoPrompt": "move"
            }]
        }))
        .unwrap();

        assert_eq!(package.defaults, ProductionPackageDefaults::default());
        assert!(package.items[0].reference_images.is_empty());
        assert!(package.items[0].reference_audios.is_empty());
        assert!(package.items[0].reference_videos.is_empty());
    }

    #[test]
    fn package_paths_fail_closed_at_the_domain_boundary() {
        for invalid in [
            "",
            "../outside.png",
            "images/../../outside.png",
            "C:\\outside.png",
            "\\\\server\\share\\outside.png",
            "/outside.png",
            "~/outside.png",
            "https://example.test/image.png",
        ] {
            assert!(
                validate_package_relative_path(invalid).is_err(),
                "path should be rejected: {invalid}"
            );
        }
        assert!(is_safe_package_relative_path("images/SH001.png"));
        assert!(is_safe_package_relative_path("references\\hero.png"));
    }

    #[test]
    fn inspection_counts_and_status_are_deterministic() {
        let ready = ProductionPackageInspectionItem {
            id: "ready".to_owned(),
            status: ProductionPackageItemStatus::Ready,
            ..Default::default()
        };
        let blocked = ProductionPackageInspectionItem {
            id: "blocked".to_owned(),
            status: ProductionPackageItemStatus::Blocked,
            errors: vec![ProductionPackageDiagnostic::blocker(
                CODE_PACKAGE_MEDIA_MISSING,
                "missing",
            )],
            ..Default::default()
        };
        let inspection = ProductionPackageInspection::from_items(
            "EP01",
            Some("external-package".to_owned()),
            vec![ready, blocked],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(inspection.item_count, 2);
        assert_eq!(inspection.ready_count, 1);
        assert_eq!(inspection.blocked_count, 1);
        assert_eq!(
            inspection.status,
            ProductionPackageInspectionStatus::Blocked
        );
    }

    #[test]
    fn execution_like_fields_are_explicitly_non_authoritative() {
        assert!(is_execution_field("workflowVersionId"));
        assert!(is_execution_field("comfyPromptId"));
        assert_eq!(
            CODE_PACKAGE_EXECUTION_FIELD_IGNORED,
            "PACKAGE_EXECUTION_FIELD_IGNORED"
        );
        assert!(is_supported_h3_generation_mode("FL2VA_FIRST_LAST"));
        assert!(!is_supported_h3_generation_mode("SECOND_EXECUTOR"));
    }
}
