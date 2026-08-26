use super::RepositoryError;
use crate::domain::consistency::ProfileType;
use crate::domain::AssetId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One human-readable relation returned by an asset/profile/reference-set
/// usage query.  The fields intentionally describe the relation rather than
/// exposing a database row, so the same DTO can be rendered by the asset
/// library and the deletion inspectors.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUsageItem {
    pub entity_type: String,
    pub entity_id: String,
    pub display_name: String,
    pub relation_type: String,
    pub scope_type: Option<String>,
    pub scope_id: Option<String>,
    pub shot_id: Option<String>,
    pub profile_type: Option<String>,
    pub reference_set_id: Option<String>,
    pub blocking: bool,
    pub detail: String,
}

impl AssetUsageItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        display_name: impl Into<String>,
        relation_type: impl Into<String>,
        scope_type: Option<String>,
        scope_id: Option<String>,
        shot_id: Option<String>,
        profile_type: Option<String>,
        reference_set_id: Option<String>,
        blocking: bool,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            display_name: display_name.into(),
            relation_type: relation_type.into(),
            scope_type,
            scope_id,
            shot_id,
            profile_type,
            reference_set_id,
            blocking,
            detail: detail.into(),
        }
    }
}

/// Project-scoped reverse usage for one physical Asset.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUsageSummary {
    pub asset_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub reference_sets: Vec<AssetUsageItem>,
    pub profiles: Vec<AssetUsageItem>,
    pub shots: Vec<AssetUsageItem>,
    /// Selected image/video keyframes are also represented in `shots`, but
    /// this dedicated bucket lets callers render the two relations directly.
    pub selected_keyframes: Vec<AssetUsageItem>,
    pub legacy_references: Vec<AssetUsageItem>,
    pub production_history: Vec<AssetUsageItem>,
    pub items: Vec<AssetUsageItem>,
}

impl AssetUsageSummary {
    pub fn new(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            ..Self::default()
        }
    }

    pub fn finish(&mut self) {
        self.items.clear();
        extend_unique(&mut self.items, &self.reference_sets);
        extend_unique(&mut self.items, &self.profiles);
        extend_unique(&mut self.items, &self.shots);
        extend_unique(&mut self.items, &self.selected_keyframes);
        extend_unique(&mut self.items, &self.legacy_references);
        extend_unique(&mut self.items, &self.production_history);
        self.total = self.items.len();
        self.blocking_count = self.items.iter().filter(|item| item.blocking).count();
    }
}

/// Project-scoped reverse usage for one semantic Profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageSummary {
    pub profile_type: String,
    pub profile_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub shot_bindings: Vec<AssetUsageItem>,
    pub scope_bindings: Vec<AssetUsageItem>,
    pub reference_sets: Vec<AssetUsageItem>,
    pub default_style_profiles: Vec<AssetUsageItem>,
    pub costume_variants: Vec<AssetUsageItem>,
    pub related_profiles: Vec<AssetUsageItem>,
    /// Wire-friendly aliases retained alongside the more explicit relation
    /// buckets above.  They keep usage responses compatible with the asset
    /// workspace without forcing callers to know storage terminology.
    pub shots: Vec<AssetUsageItem>,
    pub scopes: Vec<AssetUsageItem>,
    pub items: Vec<AssetUsageItem>,
}

impl ProfileUsageSummary {
    pub fn new(profile_type: ProfileType, profile_id: impl Into<String>) -> Self {
        Self {
            profile_type: profile_type.as_str().to_owned(),
            profile_id: profile_id.into(),
            ..Self::default()
        }
    }

    pub fn finish(&mut self) {
        if self.shots.is_empty() {
            self.shots = self.shot_bindings.clone();
        }
        if self.shot_bindings.is_empty() {
            self.shot_bindings = self.shots.clone();
        }
        if self.scopes.is_empty() {
            self.scopes = self.scope_bindings.clone();
        }
        if self.scope_bindings.is_empty() {
            self.scope_bindings = self.scopes.clone();
        }
        self.items.clear();
        extend_unique(&mut self.items, &self.shot_bindings);
        extend_unique(&mut self.items, &self.scope_bindings);
        extend_unique(&mut self.items, &self.reference_sets);
        extend_unique(&mut self.items, &self.default_style_profiles);
        extend_unique(&mut self.items, &self.costume_variants);
        extend_unique(&mut self.items, &self.related_profiles);
        self.total = self.items.len();
        self.blocking_count = self.items.iter().filter(|item| item.blocking).count();
    }
}

/// Project-scoped reverse usage for one ReferenceSet.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSetUsageSummary {
    pub reference_set_id: String,
    pub total: usize,
    pub blocking_count: usize,
    pub profile_defaults: Vec<AssetUsageItem>,
    pub costume_variants: Vec<AssetUsageItem>,
    pub shot_bindings: Vec<AssetUsageItem>,
    pub scope_bindings: Vec<AssetUsageItem>,
    pub owner: Option<AssetUsageItem>,
    pub item_count: usize,
    /// Short aliases used by the wire/UI layer.  The explicit fields remain
    /// available to application callers that need the relation category.
    pub profiles: Vec<AssetUsageItem>,
    pub costumes: Vec<AssetUsageItem>,
    pub shots: Vec<AssetUsageItem>,
    pub scopes: Vec<AssetUsageItem>,
    pub items: Vec<AssetUsageItem>,
}

impl ReferenceSetUsageSummary {
    pub fn new(reference_set_id: impl Into<String>) -> Self {
        Self {
            reference_set_id: reference_set_id.into(),
            ..Self::default()
        }
    }

    pub fn finish(&mut self) {
        if self.profiles.is_empty() {
            self.profiles = self.profile_defaults.clone();
        }
        if self.profile_defaults.is_empty() {
            self.profile_defaults = self.profiles.clone();
        }
        if self.costumes.is_empty() {
            self.costumes = self.costume_variants.clone();
        }
        if self.costume_variants.is_empty() {
            self.costume_variants = self.costumes.clone();
        }
        if self.shots.is_empty() {
            self.shots = self.shot_bindings.clone();
        }
        if self.shot_bindings.is_empty() {
            self.shot_bindings = self.shots.clone();
        }
        if self.scopes.is_empty() {
            self.scopes = self.scope_bindings.clone();
        }
        if self.scope_bindings.is_empty() {
            self.scope_bindings = self.scopes.clone();
        }
        self.items.clear();
        extend_unique(&mut self.items, &self.profile_defaults);
        extend_unique(&mut self.items, &self.costume_variants);
        extend_unique(&mut self.items, &self.shot_bindings);
        extend_unique(&mut self.items, &self.scope_bindings);
        if let Some(owner) = &self.owner {
            if !self.items.contains(owner) {
                self.items.push(owner.clone());
            }
        }
        self.total = self.items.len();
        self.blocking_count = self.items.iter().filter(|item| item.blocking).count();
    }
}

fn extend_unique(target: &mut Vec<AssetUsageItem>, values: &[AssetUsageItem]) {
    for value in values {
        if !target.contains(value) {
            target.push(value.clone());
        }
    }
}

/// Usage reads are deliberately project-scoped and single-entity.  There is
/// no batch graph API: callers ask for the one asset/profile/reference set
/// they are currently inspecting.
#[async_trait]
pub trait AssetUsageRepository: Send + Sync {
    async fn asset_usage(
        &self,
        project_id: &str,
        asset_id: &AssetId,
    ) -> Result<AssetUsageSummary, RepositoryError> {
        self.asset_usage_for(project_id, asset_id).await
    }

    /// Compatibility spelling for repository implementations and fakes that
    /// use the existing `*_for` naming convention.
    async fn asset_usage_for(
        &self,
        _project_id: &str,
        _asset_id: &AssetId,
    ) -> Result<AssetUsageSummary, RepositoryError> {
        Err(RepositoryError::database(
            "asset usage is not supported by this repository",
        ))
    }

    async fn profile_usage(
        &self,
        project_id: &str,
        profile_type: ProfileType,
        profile_id: &str,
    ) -> Result<ProfileUsageSummary, RepositoryError> {
        self.profile_usage_for(project_id, profile_type, profile_id)
            .await
    }

    /// Compatibility spelling for repository implementations and fakes that
    /// use the existing `*_for` naming convention.
    async fn profile_usage_for(
        &self,
        _project_id: &str,
        _profile_type: ProfileType,
        _profile_id: &str,
    ) -> Result<ProfileUsageSummary, RepositoryError> {
        Err(RepositoryError::database(
            "profile usage is not supported by this repository",
        ))
    }

    async fn reference_set_usage(
        &self,
        project_id: &str,
        reference_set_id: &str,
    ) -> Result<ReferenceSetUsageSummary, RepositoryError> {
        self.reference_set_usage_for(project_id, reference_set_id)
            .await
    }

    /// Compatibility spelling for repository implementations and fakes that
    /// use the existing `*_for` naming convention.
    async fn reference_set_usage_for(
        &self,
        _project_id: &str,
        _reference_set_id: &str,
    ) -> Result<ReferenceSetUsageSummary, RepositoryError> {
        Err(RepositoryError::database(
            "reference set usage is not supported by this repository",
        ))
    }
}
