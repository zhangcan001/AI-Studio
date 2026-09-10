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
    SqliteDatabaseHealthProbe, SqliteGenerationDefinitionRepository,
    SqliteGenerationSnapshotRepository, SqliteOrganizationRepository, SqlitePresetRepository,
    SqliteProductionAuditRepository, SqliteProductionItemReviewRepository,
    SqliteProductionOrchestratorRepository, SqliteProductionQueueRepository,
    SqliteProductionStructureRepository, SqliteProjectBackupRepository,
    SqliteProjectCommandCenterRepository, SqliteProjectManifestRepository, SqliteProjectRepository,
    SqliteProjectWorkflowBindingRepository, SqlitePromptLibraryRepository,
    SqliteReferenceAnchorRepository, SqliteReferenceSetRepository, SqliteScriptDraftRepository,
    SqliteScriptSourceRepository, SqliteShotConsistencyRepository, SqliteShotRepository,
    SqliteTaskHistoryRepository, SqliteTaskRepository, SqliteWorkflowBenchmarkRepository,
    SqliteWorkflowLibraryRepository, SqliteWorkflowRecipePromotionRepository,
    SqliteWorkflowRegistryRepository, SqliteWorkflowRunRepository,
    SqliteWorkflowRuntimeArtifactRepository, SqliteWorkflowRuntimeRepository,
    SqliteWorkflowRuntimeStateRepository,
};

#[cfg(test)]
pub(crate) use repositories::{
    assemble_reference_anchor_backups, DbReferenceAnchor, DbReferenceAnchorAsset,
};
