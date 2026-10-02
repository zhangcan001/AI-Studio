use super::*;
use crate::application::{
    artifact_service::{ArtifactService, ArtifactServiceError},
    asset_query_service::{AssetQueryError, AssetQueryService},
    shot_service::ShotService,
    task_history_service::{DraftValueView, TaskHistoryError, TaskHistoryService},
};
use std::collections::{BTreeMap, BTreeSet};

/// Borrow existing authorities for read composition; no Run-owned repositories.
pub struct RunDetailServices<'a> {
    pub history: &'a TaskHistoryService,
    pub assets: &'a AssetQueryService,
    pub artifacts: &'a ArtifactService,
    pub shots: &'a ShotService,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunShotContext {
    pub id: String,
    pub name: String,
    pub stage: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInput {
    pub task_id: Option<String>,
    pub item_id: Option<String>,
    pub generator_name: String,
    pub selection_ref: Option<String>,
    pub values: BTreeMap<String, DraftValueView>,
    pub reuse_unavailable_reason: Option<String>,
    pub error_message: Option<String>,
}

fn draft_value(
    value: crate::application::generation_input_preparer::GenerationInputValue,
) -> DraftValueView {
    use crate::application::generation_input_preparer::GenerationInputValue as V;
    use crate::domain::SeedValue;
    match value {
        V::Text(value) => DraftValueView::String { value },
        V::Integer(value) => DraftValueView::Integer { value },
        V::Number(value) => DraftValueView::Number { value },
        V::Seed(SeedValue::Random) => DraftValueView::SeedRandom,
        V::Seed(SeedValue::Fixed(value)) => DraftValueView::SeedFixed {
            value: value.to_string(),
        },
        V::ImageAsset(id) => DraftValueView::ImageAsset {
            asset_id: id.as_str().to_owned(),
        },
        V::VideoAsset(id) => DraftValueView::VideoAsset {
            asset_id: id.as_str().to_owned(),
        },
        V::AudioAsset(id) => DraftValueView::AudioAsset {
            asset_id: id.as_str().to_owned(),
        },
        V::ImageAssets(ids) => DraftValueView::ImageAssets {
            asset_ids: ids.into_iter().map(|id| id.as_str().to_owned()).collect(),
        },
        V::VideoAssets(ids) => DraftValueView::VideoAssets {
            asset_ids: ids.into_iter().map(|id| id.as_str().to_owned()).collect(),
        },
        V::AudioAssets(ids) => DraftValueView::AudioAssets {
            asset_ids: ids.into_iter().map(|id| id.as_str().to_owned()).collect(),
        },
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub sources: Vec<RunShotContext>,
    pub inputs: Vec<RunInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub asset_id: String,
    pub name: String,
    pub media_kind: Option<String>,
    pub asset_exists: bool,
    pub availability: Option<String>,
    pub review_state: Option<String>,
    pub review_revision: Option<i64>,
    pub selected_shot_ids: Vec<String>,
    pub thumbnail_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunResultReviewRequest {
    pub run_ref: RunRef,
    pub asset_id: String,
    pub decision: String,
    pub comment: String,
    pub expected_revision: i64,
}

fn review_error(error: ArtifactServiceError) -> ProductError {
    match error {
        ArtifactServiceError::ReviewConflict(_) => ProductError::new(
            "RUN_REVIEW_CONFLICT",
            "审核状态已更新，请刷新后重试。",
            Some("REFRESH_RUN"),
        ),
        ArtifactServiceError::ReviewInvalid(_) | ArtifactServiceError::InvalidInput(_) => {
            ProductError::new("RUN_REVIEW_INVALID", "此结果当前不能提交审核。", None)
        }
        ArtifactServiceError::NotFound(_) => {
            ProductError::new("RUN_RESULT_NOT_FOUND", "结果不存在或不属于此运行。", None)
        }
        other => ProductError::internal(other),
    }
}

impl ProductRunFacade {
    async fn task_ids(
        &self,
        project_id: &str,
        locator: &RunRef,
    ) -> Result<Vec<String>, ProductError> {
        let mut ids = BTreeSet::new();
        let mut batches = BTreeSet::new();
        match locator.source {
            RunSource::Task => {
                ids.insert(locator.id.clone());
            }
            RunSource::QueueBatch => {
                batches.insert(locator.id.clone());
            }
            RunSource::ProductionRun => {
                let run = self
                    .production
                    .get_projection(project_id, &locator.id)
                    .await
                    .map_err(production_error)?;
                for stage in run.stages {
                    if let Some(batch) = stage.production_batch_id {
                        batches.insert(batch);
                    }
                    ids.extend(stage.items.into_iter().filter_map(|item| item.task_id));
                }
            }
        }
        for id in batches {
            let batch = self.queue.get(project_id, &id).await.map_err(queue_error)?;
            // Include old attempts: a retry does not erase historical outputs.
            ids.extend(batch.items.into_iter().filter_map(|item| item.task_id));
        }
        Ok(ids.into_iter().collect())
    }

    pub async fn get_detail(
        &self,
        project_id: &str,
        locator: RunRef,
        services: &RunDetailServices<'_>,
    ) -> Result<ProductRun, ProductError> {
        let mut run = self.get(project_id, locator.clone()).await?;
        let task_ids = self.task_ids(project_id, &locator).await?;
        let mut batch_items = Vec::new();
        let batch_ids: Vec<String> = match locator.source {
            RunSource::QueueBatch => vec![locator.id.clone()],
            RunSource::ProductionRun => self
                .production
                .get_projection(project_id, &locator.id)
                .await
                .map_err(production_error)?
                .stages
                .into_iter()
                .filter_map(|stage| stage.production_batch_id)
                .collect(),
            RunSource::Task => Vec::new(),
        };
        for id in batch_ids {
            batch_items.extend(
                self.queue
                    .get(project_id, &id)
                    .await
                    .map_err(queue_error)?
                    .items,
            );
        }
        let shots = services
            .shots
            .list(project_id)
            .await
            .map_err(ProductError::internal)?;
        let mut sources = Vec::new();
        for shot in shots {
            let mut stages = BTreeSet::new();
            for link in &shot.generation_links {
                if link
                    .production_batch_item_id
                    .as_ref()
                    .is_some_and(|id| batch_items.iter().any(|item| item.id.as_str() == id))
                    || link
                        .task_id
                        .as_ref()
                        .is_some_and(|id| task_ids.contains(id))
                {
                    stages.insert(link.stage.clone());
                }
            }
            sources.extend(stages.into_iter().map(|stage| RunShotContext {
                id: shot.id.clone(),
                name: shot.name.clone(),
                stage,
            }));
        }
        let mut inputs = Vec::new();
        for id in task_ids {
            let task = services
                .history
                .get_detail(project_id, &id)
                .await
                .map_err(ProductError::internal)?;
            let draft = match services.history.get_reusable_draft(project_id, &id).await {
                Ok(draft) => Some(draft),
                Err(TaskHistoryError::DraftUnavailable(_)) => None,
                Err(other) => return Err(ProductError::internal(other)),
            };
            let reusable = draft
                .as_ref()
                .filter(|draft| draft.missing_asset_ids.is_empty());
            // A rejected historical draft must not block editing with the exact
            // generator. Create revalidates its availability and uses defaults;
            // invalid values or missing media are never blindly resubmitted.
            let selection_ref = Some(
                super::super::selection_ref::ExactGeneratorSelection {
                    workflow_version_id: task.workflow_version_id.clone(),
                    recipe_id: task.recipe_id.clone(),
                }
                .encode()?,
            );
            let reuse_unavailable_reason = reusable
                .is_none()
                .then(|| "历史输入不完整或无法安全复用，将使用生成器默认值重新编辑。".to_owned());
            let values = reusable
                .map(|draft| draft.values.clone())
                .unwrap_or_default();
            inputs.push(RunInput {
                task_id: Some(id),
                item_id: None,
                generator_name: task.workflow_name,
                selection_ref,
                values,
                reuse_unavailable_reason,
                error_message: task.error_code.as_deref().map(|code| {
                    if is_input_error(code) {
                        "输入校验失败，请检查提示词、参数与参考素材。".to_owned()
                    } else {
                        "生成未完成，请检查运行环境或高级恢复。".to_owned()
                    }
                }),
            });
        }
        for item in batch_items
            .into_iter()
            .filter(|item| item.task_id.is_none())
        {
            let values = crate::application::production_queue_service::generation_values_from_json(
                &item.values_json,
            )
            .map_err(ProductError::internal)?;
            let selection_ref = super::super::selection_ref::ExactGeneratorSelection {
                workflow_version_id: item.workflow_version_id,
                recipe_id: item.recipe_id,
            }
            .encode()?;
            inputs.push(RunInput {
                task_id: None,
                item_id: Some(item.id.as_str().to_owned()),
                generator_name: "待启动生成器".to_owned(),
                selection_ref: Some(selection_ref),
                values: values
                    .into_iter()
                    .map(|(key, value)| (key, draft_value(value)))
                    .collect(),
                reuse_unavailable_reason: None,
                error_message: item
                    .error_code
                    .map(|_| "此项未启动，请检查输入与运行环境。".to_owned()),
            });
        }
        run.detail = Some(RunDetail { sources, inputs });
        Ok(run)
    }

    pub async fn results_get(
        &self,
        project_id: &str,
        locator: RunRef,
        services: &RunDetailServices<'_>,
    ) -> Result<Vec<RunResult>, ProductError> {
        let run = self.get(project_id, locator.clone()).await?;
        let task_ids = self.task_ids(project_id, &locator).await?;
        let records = services
            .artifacts
            .task_artifacts(project_id, &task_ids)
            .await
            .map_err(review_error)?;
        let mut asset_ids = BTreeSet::from_iter(run.results_summary);
        for id in &task_ids {
            if let Some(task) = self
                .tasks
                .get(project_id, id)
                .await
                .map_err(ProductError::internal)?
            {
                asset_ids.extend(task.output_asset_ids);
            }
        }
        asset_ids.extend(records.iter().map(|record| record.id.clone()));
        let shots = services
            .shots
            .list(project_id)
            .await
            .map_err(ProductError::internal)?;
        let mut results = Vec::new();
        for id in asset_ids {
            let asset = match services.assets.get(project_id, &id).await {
                Ok(asset) => Some(asset),
                Err(AssetQueryError::NotFound(_)) => None,
                Err(error) => return Err(ProductError::internal(error)),
            };
            let record = records.iter().find(|record| record.id == id);
            results.push(RunResult {
                name: asset
                    .as_ref()
                    .map(|asset| asset.name.clone())
                    .unwrap_or_else(|| "结果已不存在".to_owned()),
                media_kind: asset.as_ref().map(|asset| asset.asset_type.clone()),
                asset_exists: asset.is_some(),
                availability: record.map(|record| record.availability.as_str().to_owned()),
                review_state: record.and_then(|record| record.review_status.clone()),
                review_revision: record.and_then(|record| record.review_revision),
                selected_shot_ids: shots
                    .iter()
                    .filter(|shot| {
                        shot.selected_image_asset_id.as_deref() == Some(&id)
                            || shot.selected_video_asset_id.as_deref() == Some(&id)
                    })
                    .map(|shot| shot.id.clone())
                    .collect(),
                asset_id: id,
                thumbnail_bytes: None,
            });
        }
        for item in results
            .iter_mut()
            .filter(|item| item.media_kind.as_deref() == Some("image"))
            .take(12)
        {
            if let Ok(binary) = services
                .assets
                .read_thumbnail(project_id, &item.asset_id)
                .await
            {
                if binary.bytes.len() <= 256 * 1024 {
                    item.thumbnail_bytes = Some(binary.bytes);
                }
            }
        }
        Ok(results)
    }

    pub async fn result_review(
        &self,
        project_id: &str,
        request: RunResultReviewRequest,
        services: &RunDetailServices<'_>,
    ) -> Result<(), ProductError> {
        let results = self
            .results_get(project_id, request.run_ref, services)
            .await?;
        if !results
            .iter()
            .any(|result| result.asset_id == request.asset_id && result.asset_exists)
        {
            return Err(ProductError::new(
                "RUN_RESULT_NOT_FOUND",
                "结果不存在或不属于此运行。",
                None,
            ));
        }
        let decision = crate::domain::ArtifactReviewDecision::parse(&request.decision)
            .map_err(|_| ProductError::new("RUN_REVIEW_INVALID", "审核决定无效。", None))?;
        services
            .artifacts
            .submit_review(
                project_id,
                &request.asset_id,
                decision,
                request.comment,
                request.expected_revision,
            )
            .await
            .map_err(review_error)?;
        Ok(())
    }
}
