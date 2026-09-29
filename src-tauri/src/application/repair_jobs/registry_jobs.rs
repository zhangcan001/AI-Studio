//! W-03 registry repair: versions stranded by the old whole-workflow remove.
//!
//! Before W-03, removing a workflow archived every version (archived=1,
//! enabled=0, archived_at=<removal time>) and restoring it only brought the
//! current version back. The workflow's `removed_at` is cleared on restore,
//! so the removal instant is recovered from the versions themselves: the old
//! remove stamped every version with the identical `archived_at`. A group is
//! repaired only when *all* non-current versions of an ACTIVE workflow are
//! archived+disabled with that one identical timestamp; anything else was
//! archived deliberately and is only reported for review.

use super::{RepairItem, RepairJob, RepairPlan, RepairSummary};
use crate::application::ports::{Clock, RegistryRepairRepository, RegistryVersionStateRecord};
use async_trait::async_trait;
use std::{collections::BTreeMap, sync::Arc};

pub const REGISTRY_STRANDED_VERSIONS_JOB_ID: &str = "registry_stranded_versions_v1";
const REASON_STRANDED: &str = "STRANDED_BY_WORKFLOW_REMOVE";
const REASON_UNMATCHED: &str = "ARCHIVED_NOT_MATCHING_REMOVAL";

/// Pure planning: returns (versions to repair, versions to report only).
pub fn plan_stranded_versions(
    records: &[RegistryVersionStateRecord],
) -> (Vec<RepairItem>, Vec<RepairItem>) {
    let mut by_workflow = BTreeMap::<&str, Vec<&RegistryVersionStateRecord>>::new();
    for record in records {
        by_workflow
            .entry(record.workflow_id.as_str())
            .or_default()
            .push(record);
    }
    let mut repair = Vec::new();
    let mut review = Vec::new();
    for (workflow_id, versions) in by_workflow {
        let stranded = versions
            .iter()
            .filter(|version| version.archived && !version.enabled)
            .copied()
            .collect::<Vec<_>>();
        if stranded.is_empty() {
            continue;
        }
        let non_current = versions
            .iter()
            .filter(|version| !version.is_current)
            .copied()
            .collect::<Vec<_>>();
        let current_restored = versions
            .iter()
            .any(|version| version.is_current && !version.archived);
        let shared_timestamp = non_current
            .first()
            .and_then(|version| version.archived_at.as_deref());
        let matches_removal = current_restored
            && shared_timestamp.is_some()
            && non_current.iter().all(|version| {
                version.archived
                    && !version.enabled
                    && version.archived_at.as_deref() == shared_timestamp
            });
        for version in stranded {
            let item = RepairItem {
                workflow_id: workflow_id.to_owned(),
                workflow_version_id: version.workflow_version_id.clone(),
                recipe_id: String::new(),
                reason: if matches_removal && !version.is_current {
                    REASON_STRANDED
                } else {
                    REASON_UNMATCHED
                }
                .to_owned(),
                workflow_version: version.workflow_version.clone(),
                recipe_version: String::new(),
                package_name: String::new(),
            };
            if matches_removal && !version.is_current {
                repair.push(item);
            } else {
                review.push(item);
            }
        }
    }
    (repair, review)
}

pub struct RegistryStrandedVersionsJob {
    repository: Arc<dyn RegistryRepairRepository>,
    clock: Arc<dyn Clock>,
}

impl RegistryStrandedVersionsJob {
    pub fn new(repository: Arc<dyn RegistryRepairRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
}

#[async_trait]
impl RepairJob for RegistryStrandedVersionsJob {
    fn id(&self) -> &'static str {
        REGISTRY_STRANDED_VERSIONS_JOB_ID
    }

    async fn plan(&self) -> Result<RepairPlan, String> {
        let records = self
            .repository
            .list_active_version_states()
            .await
            .map_err(|error| error.to_string())?;
        let (repair, review) = plan_stranded_versions(&records);
        Ok(RepairPlan {
            items: repair.into_iter().chain(review).collect(),
        })
    }

    async fn apply(&self, plan: &RepairPlan) -> Result<RepairSummary, String> {
        let (repair, review): (Vec<_>, Vec<_>) = plan
            .items
            .iter()
            .cloned()
            .partition(|item| item.reason == REASON_STRANDED);
        let ids = repair
            .iter()
            .map(|item| item.workflow_version_id.clone())
            .collect::<Vec<_>>();
        let repaired = if ids.is_empty() {
            0
        } else {
            self.repository
                .unarchive_versions(&ids, self.clock.now())
                .await
                .map_err(|error| error.to_string())? as usize
        };
        Ok(RepairSummary {
            planned: plan.items.len(),
            repaired,
            skipped: repair.len().saturating_sub(repaired),
            needs_review: review,
            failed: Vec::new(),
            published_recipe_ids: Vec::new(),
            progress: Vec::new(),
        })
    }
}
