//! The seven PR-A recipe repair jobs. Each wraps one
//! [`RecipeRepairKind`] of the onboarding service and, after a successful
//! republish, moves the recipe promotion and project workflow bindings from
//! the repaired recipe to its replacement so the fix actually takes effect.

use super::{RepairFailure, RepairItem, RepairJob, RepairPlan, RepairSummary};
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

    async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String> {
        let mut summary = RepairSummary::default();
        for item in &plan.items {
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
                    summary.repaired += 1;
                    summary.published_recipe_ids.push(new_recipe_id.clone());
                    if let (Some(workflow_version_id), Some(old_recipe_id)) =
                        (workflow_version_id, old_recipe_id)
                    {
                        if let Err(error) = self
                            .retarget(&workflow_version_id, &old_recipe_id, &new_recipe_id)
                            .await
                        {
                            summary.failed.push(RepairFailure {
                                recipe_id: item.recipe_id.clone(),
                                message: format!(
                                    "published {new_recipe_id} but could not move promotion/bindings: {error}"
                                ),
                            });
                        }
                    }
                }
                Ok(RecipeRepairOutcome::NeedsReview(reason)) => {
                    summary.needs_review.push(RepairItem {
                        reason,
                        ..item.clone()
                    });
                }
                Ok(RecipeRepairOutcome::Skipped(_)) => summary.skipped += 1,
                Err(message) => summary.failed.push(RepairFailure {
                    recipe_id: item.recipe_id.clone(),
                    message,
                }),
            }
        }
        Ok(summary)
    }
}
