pub mod pool;
pub mod repositories;

#[cfg(test)]
mod dev033_benchmark;
#[cfg(test)]
mod dev048_consistency_e2e;

pub use pool::initialize;
pub use repositories::{
    SqliteAssetBrowseRepository, SqliteAssetDeletionRepository, SqliteAssetRepository,
    SqliteAssetVideoPromptRepository, SqliteConsistencyProfileRepository,
    SqliteGenerationDefinitionRepository, SqliteGenerationSnapshotRepository,
    SqliteOrganizationRepository, SqlitePresetRepository, SqliteProductionItemReviewRepository,
    SqliteProductionQueueRepository, SqliteProductionStructureRepository,
    SqliteProjectBackupRepository, SqliteProjectRepository, SqliteProjectWorkflowBindingRepository,
    SqlitePromptLibraryRepository, SqliteReferenceAnchorRepository, SqliteReferenceSetRepository,
    SqliteScriptDraftRepository, SqliteScriptSourceRepository, SqliteShotConsistencyRepository,
    SqliteShotRepository, SqliteTaskHistoryRepository, SqliteTaskRepository,
    SqliteWorkflowLibraryRepository, SqliteWorkflowRegistryRepository, SqliteWorkflowRunRepository,
    SqliteWorkflowRuntimeArtifactRepository, SqliteWorkflowRuntimeRepository,
    SqliteWorkflowRuntimeStateRepository,
};

#[cfg(test)]
pub(crate) use repositories::{
    assemble_reference_anchor_backups, DbReferenceAnchor, DbReferenceAnchorAsset,
};
