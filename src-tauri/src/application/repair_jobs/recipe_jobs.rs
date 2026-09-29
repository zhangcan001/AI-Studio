//! The seven PR-A recipe repair jobs. Each wraps one
//! [`RecipeRepairKind`] of the onboarding service and, after a successful
//! republish, moves the recipe promotion and project workflow bindings from
//! the repaired recipe to its replacement so the fix actually takes effect.

use super::{
    RepairFailure, RepairItem, RepairItemCheckpoint, RepairItemProgress, RepairJob, RepairPlan,
    RepairSummary,
};
use crate::application::ports::{
    Clock, ProjectWorkflowBindingRepository, WorkflowRecipePromotionRepository,
};
use crate::application::workflow_onboarding_service::{
    RecipeRepairCandidate, RecipeRepairKind, RecipeRepairOutcome, WorkflowOnboardingService,
};
use async_trait::async_trait;
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct RecipeRepairJob {
    kind: RecipeRepairKind,
    onboarding: Arc<WorkflowOnboardingService>,
    promotions: Option<Arc<dyn WorkflowRecipePromotionRepository>>,
    bindings: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
    clock: Arc<dyn Clock>,
}

impl RecipeRepairJob {
    pub fn new(
        kind: RecipeRepairKind,
        onboarding: Arc<WorkflowOnboardingService>,
        promotions: Option<Arc<dyn WorkflowRecipePromotionRepository>>,
        bindings: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            kind,
            onboarding,
            promotions,
            bindings,
            clock,
        }
    }

    /// All seven jobs in registration order (the report-only publish gate
    /// recheck runs last so it sees the repaired recipes).
    pub fn all(
        onboarding: Arc<WorkflowOnboardingService>,
        promotions: Option<Arc<dyn WorkflowRecipePromotionRepository>>,
        bindings: Option<Arc<dyn ProjectWorkflowBindingRepository>>,
        clock: Arc<dyn Clock>,
    ) -> Vec<Arc<dyn RepairJob>> {
        RecipeRepairKind::ALL
            .into_iter()
            .map(|kind| {
                Arc::new(Self::new(
                    kind,
                    onboarding.clone(),
                    promotions.clone(),
                    bindings.clone(),
                    clock.clone(),
                )) as Arc<dyn RepairJob>
            })
            .collect()
    }

    async fn retarget(
        &self,
        workflow_version_id: &str,
        old_recipe_id: &str,
        new_recipe_id: &str,
    ) -> Result<(), String> {
        if let Some(promotions) = &self.promotions {
            let promoted = promotions
                .list()
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .any(|record| {
                    record.workflow_version_id == workflow_version_id
                        && record.recipe_id == old_recipe_id
                });
            if promoted {
                promotions
                    .promote(workflow_version_id, new_recipe_id, self.clock.now())
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
        if let Some(bindings) = &self.bindings {
            let projects = bindings
                .list_for_workflow_version(workflow_version_id)
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter(|record| record.recipe_id == old_recipe_id)
                .map(|record| record.project_id)
                .collect::<BTreeSet<_>>();
            for project_id in projects {
                let mut records = bindings
                    .list_for_project(&project_id)
                    .await
                    .map_err(|error| error.to_string())?;
                let now = self.clock.now();
                for record in records.iter_mut().filter(|record| {
                    record.workflow_version_id == workflow_version_id
                        && record.recipe_id == old_recipe_id
                }) {
                    record.recipe_id = new_recipe_id.to_owned();
                    record.updated_at = now;
                }
                bindings
                    .replace_for_project(&project_id, &records)
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

fn item_from_candidate(candidate: RecipeRepairCandidate) -> RepairItem {
    RepairItem {
        workflow_id: candidate.workflow_id,
        workflow_version_id: String::new(),
        recipe_id: candidate.recipe_id,
        reason: candidate.reason,
        workflow_version: candidate.workflow_version,
        recipe_version: candidate.recipe_version,
        package_name: candidate.package_name,
    }
}

fn candidate_from_item(item: &RepairItem) -> RecipeRepairCandidate {
    RecipeRepairCandidate {
        workflow_id: item.workflow_id.clone(),
        workflow_version: item.workflow_version.clone(),
        recipe_version: item.recipe_version.clone(),
        recipe_id: item.recipe_id.clone(),
        package_name: item.package_name.clone(),
        reason: item.reason.clone(),
    }
}

fn same_repair_item(left: &RepairItem, right: &RepairItem) -> bool {
    left.workflow_id == right.workflow_id
        && left.workflow_version_id == right.workflow_version_id
        && left.recipe_id == right.recipe_id
        && left.workflow_version == right.workflow_version
        && left.recipe_version == right.recipe_version
        && left.package_name == right.package_name
}

fn upsert_progress(summary: &mut RepairSummary, progress: RepairItemProgress) {
    if let Some(existing) = summary
        .progress
        .iter_mut()
        .find(|existing| same_repair_item(&existing.item, &progress.item))
    {
        *existing = progress;
    } else {
        summary.progress.push(progress);
    }
}

fn push_unique_recipe_id(summary: &mut RepairSummary, recipe_id: &str) {
    if !summary
        .published_recipe_ids
        .iter()
        .any(|published| published == recipe_id)
    {
        summary.published_recipe_ids.push(recipe_id.to_owned());
    }
}

fn has_needs_review(summary: &RepairSummary, item: &RepairItem) -> bool {
    summary
        .needs_review
        .iter()
        .any(|review| same_repair_item(review, item))
}

#[async_trait]
impl RepairJob for RecipeRepairJob {
    fn id(&self) -> &'static str {
        self.kind.job_id()
    }

    async fn plan(&self) -> Result<RepairPlan, String> {
        let promoted = match &self.promotions {
            Some(promotions) => promotions
                .list()
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|record| record.recipe_id)
                .collect(),
            None => BTreeSet::new(),
        };
        let candidates = self
            .onboarding
            .plan_recipe_repairs(self.kind, &promoted)
            .await?;
        Ok(RepairPlan {
            items: candidates.into_iter().map(item_from_candidate).collect(),
        })
    }

    async fn plan_with_previous(
        &self,
        previous: Option<&RepairSummary>,
    ) -> Result<RepairPlan, String> {
        let mut plan = self.plan().await?;
        let Some(previous) = previous else {
            return Ok(plan);
        };

        // A source package can disappear or change its candidate ordering
        // after a failed run. Re-add every non-terminal checkpoint from the
        // persisted summary so retry does not depend on a fresh scan finding
        // the same item again. Human-review items are deliberately excluded.
        let mut seen = plan.items.iter().cloned().collect::<Vec<_>>();
        plan.items.retain(|item| {
            !has_needs_review(previous, item)
                && !previous.progress.iter().any(|progress| {
                    same_repair_item(&progress.item, item)
                        && progress.checkpoint == RepairItemCheckpoint::Retargeted
                })
        });
        for progress in &previous.progress {
            if progress.checkpoint == RepairItemCheckpoint::Retargeted
                || has_needs_review(previous, &progress.item)
                || seen
                    .iter()
                    .any(|item| same_repair_item(item, &progress.item))
            {
                continue;
            }
            seen.push(progress.item.clone());
            plan.items.push(progress.item.clone());
        }
        Ok(plan)
    }

    async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String> {
        self.apply_with_previous(plan, None).await
    }

    async fn apply_with_previous(
        &self,
        plan: &RepairPlan,
        previous: Option<&RepairSummary>,
    ) -> Result<RepairSummary, String> {
        let mut summary = RepairSummary::default();
        if let Some(previous) = previous {
            summary.repaired = previous.repaired;
            summary.skipped = previous.skipped;
            summary.needs_review = previous.needs_review.clone();
            summary.published_recipe_ids = previous.published_recipe_ids.clone();
            summary.progress = previous.progress.clone();
        }
        for item in &plan.items {
            let previous_progress = summary
                .progress
                .iter()
                .find(|progress| same_repair_item(&progress.item, item))
                .cloned();
            if previous_progress
                .as_ref()
                .is_some_and(|progress| progress.checkpoint == RepairItemCheckpoint::Retargeted)
            {
                continue;
            }

            if let Some(progress) = previous_progress
                .as_ref()
                .filter(|progress| progress.checkpoint == RepairItemCheckpoint::Published)
            {
                let Some(new_recipe_id) = progress.new_recipe_id.as_deref() else {
                    summary.failed.push(RepairFailure {
                        recipe_id: item.recipe_id.clone(),
                        message: "published checkpoint is missing new recipe id".to_owned(),
                    });
                    continue;
                };
                let retarget_result = match (
                    progress.workflow_version_id.as_deref(),
                    progress.old_recipe_id.as_deref(),
                ) {
                    (Some(workflow_version_id), Some(old_recipe_id)) => {
                        self.retarget(workflow_version_id, old_recipe_id, new_recipe_id)
                            .await
                    }
                    _ => Ok(()),
                };
                match retarget_result {
                    Ok(()) => {
                        upsert_progress(
                            &mut summary,
                            RepairItemProgress {
                                item: item.clone(),
                                checkpoint: RepairItemCheckpoint::Retargeted,
                                workflow_version_id: progress.workflow_version_id.clone(),
                                old_recipe_id: progress.old_recipe_id.clone(),
                                new_recipe_id: progress.new_recipe_id.clone(),
                                last_error: None,
                            },
                        );
                        summary.repaired += 1;
                    }
                    Err(error) => {
                        let message = format!(
                            "published {new_recipe_id} but could not move promotion/bindings: {error}"
                        );
                        upsert_progress(
                            &mut summary,
                            RepairItemProgress {
                                item: item.clone(),
                                checkpoint: RepairItemCheckpoint::Published,
                                workflow_version_id: progress.workflow_version_id.clone(),
                                old_recipe_id: progress.old_recipe_id.clone(),
                                new_recipe_id: progress.new_recipe_id.clone(),
                                last_error: Some(message.clone()),
                            },
                        );
                        summary.failed.push(RepairFailure {
                            recipe_id: item.recipe_id.clone(),
                            message,
                        });
                    }
                }
                continue;
            }

            match self
                .onboarding
                .apply_recipe_repair(self.kind, &candidate_from_item(item))
                .await
            {
                Ok(RecipeRepairOutcome::Published {
                    workflow_version_id,
                    old_recipe_id,
                    new_recipe_id,
                }) => {
                    push_unique_recipe_id(&mut summary, &new_recipe_id);
                    upsert_progress(
                        &mut summary,
                        RepairItemProgress {
                            item: item.clone(),
                            checkpoint: RepairItemCheckpoint::Published,
                            workflow_version_id: workflow_version_id.clone(),
                            old_recipe_id: old_recipe_id.clone(),
                            new_recipe_id: Some(new_recipe_id.clone()),
                            last_error: None,
                        },
                    );
                    let retarget_result =
                        match (workflow_version_id.as_deref(), old_recipe_id.as_deref()) {
                            (Some(workflow_version_id), Some(old_recipe_id)) => {
                                self.retarget(workflow_version_id, old_recipe_id, &new_recipe_id)
                                    .await
                            }
                            _ => Ok(()),
                        };
                    match retarget_result {
                        Ok(()) => {
                            upsert_progress(
                                &mut summary,
                                RepairItemProgress {
                                    item: item.clone(),
                                    checkpoint: RepairItemCheckpoint::Retargeted,
                                    workflow_version_id: workflow_version_id.clone(),
                                    old_recipe_id: old_recipe_id.clone(),
                                    new_recipe_id: Some(new_recipe_id.clone()),
                                    last_error: None,
                                },
                            );
                            summary.repaired += 1;
                        }
                        Err(error) => {
                            let message = format!(
                                "published {new_recipe_id} but could not move promotion/bindings: {error}"
                            );
                            if let Some(progress) = summary
                                .progress
                                .iter_mut()
                                .find(|progress| same_repair_item(&progress.item, item))
                            {
                                progress.last_error = Some(message.clone());
                            }
                            summary.failed.push(RepairFailure {
                                recipe_id: item.recipe_id.clone(),
                                message,
                            });
                        }
                    }
                }
                Ok(RecipeRepairOutcome::NeedsReview(reason)) => {
                    let review = RepairItem {
                        reason,
                        ..item.clone()
                    };
                    if !has_needs_review(&summary, &review) {
                        summary.needs_review.push(review);
                    }
                }
                Ok(RecipeRepairOutcome::Skipped(_)) => summary.skipped += 1,
                Err(message) => {
                    upsert_progress(
                        &mut summary,
                        RepairItemProgress {
                            item: item.clone(),
                            checkpoint: RepairItemCheckpoint::Planned,
                            workflow_version_id: None,
                            old_recipe_id: None,
                            new_recipe_id: None,
                            last_error: Some(message.clone()),
                        },
                    );
                    summary.failed.push(RepairFailure {
                        recipe_id: item.recipe_id.clone(),
                        message,
                    });
                }
            }
        }
        Ok(summary)
    }
}
