//! Stateless orchestration over the existing Shot preparation and Production Queue.
use super::ProductCreationFacade;
use crate::application::{
    generation_input_preparer::GenerationInputValue,
    generation_service::GenerationServiceError,
    product::{
        error::ProductError,
        run_facade::{RunRef, RunSource},
        selection_ref::ExactGeneratorSelection,
    },
    production_queue_service::{
        CreateDirectGenerationRequest, CreateProductionBatchItem, ExecutionType,
        ProductionQueueService,
    },
    shot_service::{ShotGenerationRequest, ShotServiceError},
};
use crate::domain::{ProductionBatchStatus, ShotStage};
use serde::Serialize;
use std::{collections::BTreeMap, future::Future};

pub struct CreationSubmission {
    pub prompt_id: Option<String>,
    pub prompt_version_id: Option<String>,
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub selection_ref: String,
    pub values: BTreeMap<String, GenerationInputValue>,
    pub submission_idempotency_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationReadiness {
    pub ready: bool,
    pub issues: Vec<ProductError>,
    pub field_errors: Vec<ProductError>,
    pub actions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationAccepted {
    pub accepted: bool,
    pub run_ref: RunRef,
    pub start_outcome: &'static str,
    pub start_issue: Option<ProductError>,
}

impl ProductCreationFacade {
    /// Shared preparation is rerun on generate. A readiness response is not an authorization.
    async fn prepare_submission(
        &self,
        queue: &ProductionQueueService,
        prompts: &crate::application::prompt_library_service::PromptLibraryService,
        request: CreationSubmission,
    ) -> Result<CreateDirectGenerationRequest, ProductError> {
        if request.submission_idempotency_key.trim().is_empty()
            || request.submission_idempotency_key.len() > 256
        {
            return Err(ProductError::new(
                "INVALID_INPUT",
                "提交标识无效。",
                Some("EDIT_INPUT"),
            ));
        }
        let prompt_version_id = match (&request.prompt_id, &request.prompt_version_id) {
            (None, None) => None,
            (Some(prompt_id), Some(version_id)) => {
                let entry = prompts
                    .get(&request.project_id, prompt_id)
                    .await
                    .map_err(|_| prompt_provenance_error())?;
                let version = entry
                    .versions
                    .iter()
                    .find(|v| v.id == *version_id)
                    .ok_or_else(prompt_provenance_error)?;
                if entry.project_id != request.project_id
                    || version.prompt_id != *prompt_id
                    || !matches!(request.values.get("prompt"), Some(GenerationInputValue::Text(text)) if text == &version.text)
                {
                    return Err(prompt_provenance_error());
                }
                Some(version_id.clone())
            }
            _ => return Err(prompt_provenance_error()),
        };
        let stage = match request.stage.as_str() {
            "image" => ShotStage::Image,
            "video" => ShotStage::Video,
            _ => {
                return Err(ProductError::new(
                    "INVALID_INPUT",
                    "请选择图片或视频阶段。",
                    Some("EDIT_INPUT"),
                ))
            }
        };
        self.bindings
            .get(&request.project_id)
            .await
            .map_err(ProductError::binding)?;
        let selection = ExactGeneratorSelection::decode(&request.selection_ref)?;
        if !self
            .bindings
            .is_workflow_available_for_recipe(&selection.workflow_version_id, &selection.recipe_id)
            .await
            .map_err(ProductError::binding)?
        {
            return Err(ProductError::invalid_selection());
        }
        let prepared = self
            .shots
            .prepare_creation_submission(
                ShotGenerationRequest {
                    project_id: request.project_id,
                    shot_id: request.shot_id,
                    stage,
                    values: request.values,
                    retry_task_id: None,
                    submission_idempotency_key: Some(request.submission_idempotency_key),
                },
                selection.workflow_version_id,
                selection.recipe_id,
            )
            .await
            .map_err(|error| match error {
                ShotServiceError::NotFound(_) => {
                    ProductError::new("PROJECT_SCOPE_VIOLATION", "无法访问当前镜头。", None)
                }
                ShotServiceError::InvalidInput(_) => ProductError::new(
                    "INVALID_INPUT",
                    "镜头或生成器配置无效。",
                    Some("EDIT_INPUT"),
                ),
                other => ProductError::internal(other),
            })?;
        let item = CreateProductionBatchItem {
            workflow_version_id: prepared.workflow_version_id,
            recipe_id: prepared.recipe_id,
            values: prepared.values,
        };
        // Existing compiler/preparer owns required/type/range/cardinality, asset
        // existence/project/type/file checks and current runtime schema validation.
        queue
            .preflight_direct_generation(&prepared.project_id, &item)
            .await
            .map_err(generation_error)?;
        Ok(CreateDirectGenerationRequest {
            project_id: prepared.project_id,
            name: "镜头生成".to_owned(),
            continue_on_failure: true,
            item,
            shot_id: Some(prepared.shot_id),
            stage: Some(stage.as_str().to_owned()),
            prompt_version_id,
            model_version_id: None,
            tool_instance_id: None,
            tool_version_id: None,
            execution_type: ExecutionType::Direct,
            submission_idempotency_key: prepared.submission_idempotency_key,
            parent_task_id: None,
            execution_input_sources: None,
        })
    }

    pub async fn readiness_get(
        &self,
        queue: &ProductionQueueService,
        prompts: &crate::application::prompt_library_service::PromptLibraryService,
        request: CreationSubmission,
    ) -> CreationReadiness {
        let result = self.prepare_submission(queue, prompts, request).await;
        let error = match result {
            Err(error) => Some(error),
            Ok(_) => match queue.admission_status().await {
                Ok(status) if status.busy => Some(ProductError::new(
                    "RUNTIME_BLOCKED",
                    "运行资源正在使用中，请稍后生成。",
                    Some("TRY_LATER"),
                )),
                Ok(_) => None,
                Err(error) => Some(ProductError::internal(error)),
            },
        };
        let issues: Vec<_> = error.into_iter().collect();
        CreationReadiness {
            ready: issues.is_empty(),
            field_errors: issues
                .iter()
                .filter(|issue| issue.details.field.is_some())
                .cloned()
                .collect(),
            actions: issues
                .iter()
                .filter_map(|issue| issue.details.action.map(str::to_owned))
                .collect(),
            issues,
        }
    }

    /// `start` is the existing admission service, injected to keep Tauri outside
    /// application orchestration. No ledger, queue, executor or state machine here.
    pub async fn generate<F, Fut>(
        &self,
        queue: &ProductionQueueService,
        prompts: &crate::application::prompt_library_service::PromptLibraryService,
        request: CreationSubmission,
        start: F,
    ) -> Result<CreationAccepted, ProductError>
    where
        F: FnOnce(String, String) -> Fut,
        Fut: Future<Output = Result<(), ProductError>>,
    {
        let prepared = self.prepare_submission(queue, prompts, request).await?;
        let project_id = prepared.project_id.clone();
        let detail = queue
            .create_direct_generation(prepared)
            .await
            .map_err(ProductError::internal)?;
        let id = detail.batch.id.as_str().to_owned();
        let run_ref = RunRef {
            source: RunSource::QueueBatch,
            id: id.clone(),
        };
        // Existing queue key-set dedup returns the original batch. Never restart
        // an already running/completed batch just because the request was retried.
        if detail.batch.status != ProductionBatchStatus::Ready {
            return Ok(CreationAccepted {
                accepted: true,
                run_ref,
                start_outcome: "ALREADY_ACCEPTED",
                start_issue: None,
            });
        }
        let (start_outcome, start_issue) = match start(project_id, id).await {
            Ok(()) => ("STARTED", None),
            Err(error) => ("FAILED_TO_START", Some(error)),
        };
        // Persistence succeeded even if admission/start failed. Never lose RunRef.
        Ok(CreationAccepted {
            accepted: true,
            run_ref,
            start_outcome,
            start_issue,
        })
    }
}

fn prompt_provenance_error() -> ProductError {
    let mut error = ProductError::new(
        "INVALID_INPUT",
        "提示词来源与当前正文不一致，请重新选择提示词或继续作为手工文本编辑。",
        Some("EDIT_INPUT"),
    );
    error.details.field = Some("prompt".into());
    error
}

fn generation_error(error: GenerationServiceError) -> ProductError {
    use crate::application::generation_input_preparer::GenerationInputPrepareError as P;
    use crate::compiler::CompileError as C;
    let (code, field) = match &error {
        GenerationServiceError::DefinitionNotFound { .. } => ("GENERATOR_UNAVAILABLE", None),
        GenerationServiceError::Compile(compile) => match compile {
            C::InputRequired { input } => ("MISSING_INPUT", Some(input.clone())),
            C::UnknownInput { input } | C::InputTypeMismatch { input, .. } => {
                ("INPUT_TYPE_MISMATCH", Some(input.clone()))
            }
            C::InputOutOfRange { input, .. }
            | C::InputStepMismatch { input, .. }
            | C::InputNumberOutOfRange { input, .. }
            | C::InputNumberStepMismatch { input, .. }
            | C::InputCountOutOfRange { input, .. }
            | C::SeedOutOfRange { input, .. } => ("INPUT_OUT_OF_RANGE", Some(input.clone())),
            _ => ("GENERATOR_UNAVAILABLE", None),
        },
        GenerationServiceError::InputPrepare(P::AssetProjectMismatch { .. }) => {
            ("ASSET_PROJECT_MISMATCH", None)
        }
        GenerationServiceError::InputPrepare(
            P::AssetTypeInvalid { .. } | P::InvalidAssetMime { .. },
        ) => ("ASSET_TYPE_MISMATCH", None),
        GenerationServiceError::InputPrepare(P::AssetNotFound { .. } | P::AssetRead { .. }) => {
            ("ASSET_UNAVAILABLE", None)
        }
        GenerationServiceError::InputPrepare(_) => ("INVALID_INPUT", None),
        GenerationServiceError::ExecutionFailed { code, .. }
            if code == "WORKFLOW_UNAVAILABLE_FOR_NEW_GENERATION" =>
        {
            ("GENERATOR_UNAVAILABLE", None)
        }
        GenerationServiceError::ExecutionFailed { .. } | GenerationServiceError::Comfy(_) => {
            ("RUNTIME_BLOCKED", None)
        }
        _ => ("INTERNAL_ERROR", None),
    };
    let mut result = ProductError::new(code, "请检查输入或运行环境后重试。", Some("EDIT_INPUT"));
    if code == "RUNTIME_BLOCKED" {
        result.details.action = Some("OPEN_RUNTIME_SETTINGS");
    }
    if code == "GENERATOR_UNAVAILABLE" {
        result.details.action = Some("SELECT_GENERATOR");
    }
    result.details.field = field;
    result.details.technical_details = Some(error.to_string());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{application::ports::ComfyAdapterError, compiler::CompileError};

    #[test]
    fn product_creation_submission_typed_runtime_and_field_actions() {
        for error in [
            GenerationServiceError::Comfy(ComfyAdapterError::Offline("private endpoint".into())),
            GenerationServiceError::ExecutionFailed {
                code: "RUNTIME_CHECK_FAILED".into(),
                message: "private backend detail".into(),
                details: None,
            },
        ] {
            let issue = generation_error(error);
            assert_eq!(issue.code, "RUNTIME_BLOCKED");
            assert_eq!(issue.details.action, Some("OPEN_RUNTIME_SETTINGS"));
            assert!(issue.details.field.is_none());
            assert_eq!(
                serde_json::to_value(issue).unwrap()["details"]["action"],
                "OPEN_RUNTIME_SETTINGS"
            );
        }
        let issue = generation_error(GenerationServiceError::Compile(
            CompileError::InputOutOfRange {
                input: "width".into(),
                value: 9999,
                min: Some(64),
                max: Some(1920),
            },
        ));
        assert_eq!(issue.code, "INPUT_OUT_OF_RANGE");
        assert_eq!(issue.details.action, Some("EDIT_INPUT"));
        assert_eq!(issue.details.field.as_deref(), Some("width"));
    }
}
