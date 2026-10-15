pub mod artifact_service;
pub mod asset_data_service;
pub mod asset_deletion_service;
pub mod asset_import_service;
pub mod asset_library_service;
pub mod asset_query_service;
pub mod asset_usage_service;
pub mod asset_video_prompt_service;
pub mod batch_workflow_preset_service;
pub mod build_info;
pub mod builtin_runtime_packages;
#[cfg(test)]
mod cancellation_e2e;
mod comfy_execution_failure;
pub mod comfy_memory_service;
pub mod comfy_preflight_service;
pub mod comfy_service;
pub mod consistency_profile_service;
pub mod consistency_scope_binding_service;
#[cfg(test)]
mod dev027_e2e;
#[cfg(test)]
mod dev031_e2e;
pub mod diagnostics_service;
pub mod episode_production_service;
pub mod external_production_handoff_service;
pub mod generation_catalog_service;
#[cfg(test)]
mod generation_e2e;
pub mod generation_input_preparer;
pub mod generation_service;
pub mod h3_local_import_service;
pub(crate) mod image_inspection;
pub mod media_probe;
pub mod media_protocol;
pub mod minimax_video_product_policy;
pub mod model_service;
pub(crate) mod ordered_reference_binding;
pub mod organization_service;
pub mod output_collector;
pub mod pagination;
pub mod ports;
pub mod preset_service;
pub mod product;
pub mod product_runtime_scope;
pub mod production_audit_service;
pub mod production_batch_runbook_service;
pub mod production_orchestrator_service;
pub mod production_package_discovery_service;
pub mod production_package_inspector;
pub mod production_package_service;
pub mod production_preparation_service;
pub mod production_queue_service;
pub mod production_start_admission_service;
pub mod production_structure_service;
pub mod project_backup_service;
pub mod project_bootstrap;
pub mod project_command_center_service;
pub mod project_manifest_service;
pub mod project_service;
pub mod project_template_service;
pub mod project_workflow_binding_service;
pub mod prompt_context_builder;
pub mod prompt_library_service;
pub mod provenance_lineage_service;
pub mod recipe_history_query_service;
pub mod reference_anchor_service;
pub mod reference_set_service;
pub mod repair_jobs;
pub mod scene_production_service;
pub mod scheduler;
pub mod series_production_service;
pub mod settings_service;
pub mod shot_batch_service;
pub mod shot_bulk_service;
pub mod shot_consistency_binding_service;
pub mod shot_context_resolver;
pub mod shot_readiness_evaluator;
pub mod shot_readiness_service;
pub mod shot_reference_pack_builder;
pub mod shot_service;
pub mod shot_video_input_service;
pub(crate) mod shot_workflow_compatibility;
pub mod source_asset_import_service;
// Inlined (no new backend source files under the frozen Phase2/3 aggregate).
pub mod story_entity_service {
    use crate::application::ports::{RepositoryError, StoryEntityRepository};
    use crate::domain::story_entity::{StoryEntity, StoryEntityKind, StoryEntityUpsert};
    use crate::error::AppError;
    use std::sync::Arc;

    pub struct StoryEntityService {
        repo: Arc<dyn StoryEntityRepository>,
    }

    impl StoryEntityService {
        pub fn new(repo: Arc<dyn StoryEntityRepository>) -> Self {
            Self { repo }
        }

        pub async fn list(
            &self,
            project_id: &str,
            kind: Option<StoryEntityKind>,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo.list(project_id, kind).await.map_err(map_repo)
        }

        pub async fn get(&self, project_id: &str, id: &str) -> Result<StoryEntity, AppError> {
            match self.repo.get(project_id, id).await.map_err(map_repo)? {
                Some(entity) => Ok(entity),
                None => Err(AppError::invalid_input(format!(
                    "story entity not found: {id}"
                ))),
            }
        }

        pub async fn create(
            &self,
            project_id: &str,
            kind: StoryEntityKind,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, AppError> {
            self.repo
                .create(project_id, kind, input)
                .await
                .map_err(map_repo)
        }

        pub async fn update(
            &self,
            project_id: &str,
            id: &str,
            input: StoryEntityUpsert,
        ) -> Result<StoryEntity, AppError> {
            self.repo
                .update(project_id, id, input)
                .await
                .map_err(map_repo)
        }

        pub async fn delete(&self, project_id: &str, id: &str) -> Result<(), AppError> {
            self.repo.delete(project_id, id).await.map_err(map_repo)
        }

        pub async fn list_for_shot(
            &self,
            project_id: &str,
            shot_id: &str,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo
                .list_for_shot(project_id, shot_id)
                .await
                .map_err(map_repo)
        }

        pub async fn replace_shot_bindings(
            &self,
            project_id: &str,
            shot_id: &str,
            entity_ids: Vec<String>,
        ) -> Result<Vec<StoryEntity>, AppError> {
            self.repo
                .replace_shot_bindings(project_id, shot_id, &entity_ids)
                .await
                .map_err(map_repo)
        }
    }

    fn map_repo(error: RepositoryError) -> AppError {
        match error {
            RepositoryError::NotFound { entity, id } => {
                AppError::invalid_input(format!("{entity} not found: {id}"))
            }
            RepositoryError::Integrity { message } => AppError::invalid_input(message),
            RepositoryError::Database { message } => AppError::database(message),
            RepositoryError::Serialization { context, message } => {
                AppError::database(format!("{context}: {message}"))
            }
            other => AppError::database(format!("{other:?}")),
        }
    }
}
pub mod task_cancellation_service;
pub mod task_execution_registry;
pub mod task_history_service;
pub mod task_query_service;
pub mod task_recovery_service;
pub mod tool_service;
pub mod workflow_analysis_service;
pub mod workflow_benchmark_service;
pub mod workflow_graph_analysis;
pub mod workflow_library_service;
pub mod workflow_lifecycle_coordinator;
pub mod workflow_lifecycle_service;
pub mod workflow_manifest;
pub mod workflow_onboarding_service;
pub mod workflow_recognition_provenance;
pub mod workflow_recognition_schema;
pub mod workflow_recognition_service;
#[cfg(test)]
mod workflow_recognition_v3_replay;
pub mod workflow_registry_service;
pub mod workflow_semantic_graph;
pub mod workflow_semantic_identity;
pub mod workflow_ui_compatibility;
pub mod workflow_ui_normalizer;
pub mod workflow_ui_serialization;
pub mod workflow_workspace_query_service;

pub mod workflow_file_service;
