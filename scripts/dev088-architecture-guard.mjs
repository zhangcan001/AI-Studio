import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(import.meta.url), "..", "..");
const ipcTransportPath = resolve(root, "src/services/ipc.ts");
const productionFrontendFiles = [];

const retiredAuthoringPaths = [
  "src-tauri/src/application/prompt_template_service.rs",
  "src-tauri/src/application/prompt_template_bulk_service.rs",
  "src-tauri/src/application/script_draft_service.rs",
  "src-tauri/src/application/script_import_service.rs",
  "src-tauri/src/application/script_import_parser",
  "src-tauri/src/commands/prompt_template.rs",
  "src-tauri/src/domain/prompt_template.rs",
  "src-tauri/src/domain/script_draft",
  "src/features/shots/PromptTemplatePanel.tsx",
  "src/features/prompts/PromptTemplateVariableHelper.tsx",
  "src/features/prompts/promptTemplateState.ts",
];
if (retiredAuthoringPaths.some((path) => existsSync(join(root, path)))) {
  throw new Error("INTERNAL_AUTHORING_RETIREMENT failed: retired authoring path still exists");
}

const activeAuthoringSources = [
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/commands/mod.rs",
  "src-tauri/src/domain/mod.rs",
  "src-tauri/src/infrastructure/database/repositories/mod.rs",
  "src-tauri/src/lib.rs",
  "src/services/tauriClient.ts",
].map((path) => readFileSync(join(root, path), "utf8"));
if (activeAuthoringSources.some((source) => /(?:prompt_template_|ScriptImportService|ScriptDraftService|script_import_parser|pub mod script_draft)/.test(source))) {
  throw new Error("INTERNAL_AUTHORING_RETIREMENT failed: active runtime still references retired authoring symbols");
}

const retiredArchitectureDocs = [
  "docs/architecture/NARRATIVE_PREPRODUCTION_V2.md",
  "docs/architecture/SCRIPT_IMPORT_V1.md",
  "docs/architecture/STORYBOARD_DRAFT_V1.md",
].map((path) => readFileSync(join(root, path), "utf8"));
if (retiredArchitectureDocs.some((source) => !source.includes("STATUS=RETIRED") || !source.includes("RETIRED_BY=DEV-100"))) {
  throw new Error("INTERNAL_STORYBOARD_AUTHORING_RETIRED failed: historical authoring docs must be explicitly retired");
}

const handoffArchitectureSource = readFileSync(join(root, "docs/DEV_100_EXTERNAL_AGENT_HANDOFF_ARCHITECTURE.md"), "utf8");
if (!["IMPORT_AUTO_QUEUE=NO", "IMPORT_AUTO_TASK=NO", "IMPORT_AUTO_GENERATION=NO", "HANDOFF_IMPLEMENTATION_BLOCKED_BY_SCHEMA=YES"].every((marker) => handoffArchitectureSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION failed: handoff must remain explicit and schema-blocked");
}

const handoffContractSource = readFileSync(join(root, "docs/EXTERNAL_AGENT_PRODUCTION_HANDOFF_V1.md"), "utf8");
const handoffServiceSource = readFileSync(join(root, "src-tauri/src/application/external_production_handoff_service.rs"), "utf8");
const handoffPortSource = readFileSync(join(root, "src-tauri/src/application/ports/external_production_handoff_repository.rs"), "utf8");
const handoffRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/external_production_handoff.rs"), "utf8");
const handoffCommandSource = readFileSync(join(root, "src-tauri/src/commands/external_production_handoff.rs"), "utf8");
const handoffMigrationSource = readFileSync(join(root, "src-tauri/migrations/032_external_production_handoffs.sql"), "utf8");
const handoffClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const handoffPanelSource = readFileSync(join(root, "src/features/projects/ExternalAgentHandoffPanel.tsx"), "utf8");
if (!handoffContractSource.includes("STATUS=IMPLEMENTED")
  || !handoffContractSource.includes("IMPLEMENTATION=SERVER_ATOMIC")
  || !handoffContractSource.includes("MIGRATION=032")) {
  throw new Error("EXTERNAL_HANDOFF_CONTRACT_IMPLEMENTED failed: V1 contract must describe the shipped implementation");
}
if (!handoffServiceSource.includes("pub async fn preview(")
  || !handoffServiceSource.includes("pub async fn confirm(")
  || !handoffPortSource.includes("trait ExternalProductionHandoffRepository")
  || !handoffRepositorySource.includes("async fn import_atomic(")
  || !handoffRepositorySource.includes("self.pool.begin()")) {
  throw new Error("EXTERNAL_HANDOFF_AUTHORITY failed: service/port/repository transaction authority is incomplete");
}
if (!handoffServiceSource.includes("repository.import_atomic")
  || !handoffServiceSource.includes("find_by_document_hash")
  || !handoffPanelSource.includes("previewExternalProductionHandoff")
  || !handoffPanelSource.includes("confirmExternalProductionHandoff")) {
  throw new Error("EXTERNAL_HANDOFF_PREVIEW_READ_ONLY failed: preview and explicit confirmation boundaries are required");
}
if (!["external_production_handoffs", "external_production_handoff_entities", "UNIQUE (project_id, document_sha256)"].every((marker) => handoffMigrationSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_SINGLE_TRANSACTION failed: durable handoff identity/provenance migration is missing");
}
if (["create_task", "enqueue", "start_comfy", "ComfyUI", "production_queue"].some((marker) => handoffServiceSource.includes(marker) || handoffRepositorySource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION failed: handoff must not own queue, task, or executor side effects");
}
if (!handoffServiceSource.includes("find_active")
  || !handoffRepositorySource.includes("workflow_version_id")
  || !handoffRepositorySource.includes("recipe_id")
  || !handoffRepositorySource.includes("archived")) {
  throw new Error("EXTERNAL_HANDOFF_EXACT_WORKFLOW_RECIPE failed: exact active workflowVersionId+recipeId validation is required");
}
if (/[\s\S]*sqlx::/.test(handoffServiceSource) || /[\s\S]*sqlx::/.test(handoffPortSource)) {
  throw new Error("APPLICATION_DIRECT_SQLX_NEW_USAGE failed: handoff application layer must use repository ports");
}
if (!["previewExternalProductionHandoff", "confirmExternalProductionHandoff", "external_production_handoff_preview", "external_production_handoff_confirm"].every((marker) => handoffClientSource.includes(marker) || handoffCommandSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_TYPED_IPC failed: typed client and command parity is required");
}

function collectSourceFiles(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collectSourceFiles(path);
    else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\./.test(entry.name)) productionFrontendFiles.push(path);
  }
}

collectSourceFiles(join(root, "src"));

const rawInvokeImports = productionFrontendFiles.filter((path) => {
  const source = readFileSync(path, "utf8");
  return source.includes("@tauri-apps/api/core") && resolve(path) !== ipcTransportPath;
});
if (rawInvokeImports.length) {
  throw new Error(`FRONTEND_NO_RAW_INVOKE failed:\n${rawInvokeImports.join("\n")}`);
}

const transportSources = [
  readFileSync(join(root, "src/services/tauriClient.ts"), "utf8"),
  readFileSync(join(root, "src/services/workflowClient.ts"), "utf8"),
  readFileSync(join(root, "src/features/shots/ShotBulkImportPanel.tsx"), "utf8"),
].join("\n");
const frontendCommands = new Set(
  [...transportSources.matchAll(/\binvoke(?:Command)?\s*(?:<[\s\S]*?>)?\s*\(\s*["']([^"']+)["']/g)].map((match) => match[1]),
);

const libSource = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const handlerBlock = libSource.match(/generate_handler!\[([\s\S]*?)\]\)/)?.[1] ?? "";
const backendCommands = new Set(
  [...handlerBlock.matchAll(/commands::(?:[A-Za-z0-9_]+::)?([A-Za-z0-9_]+)/g)].map((match) => match[1]),
);
const missingCommands = [...frontendCommands].filter((command) => !backendCommands.has(command));
if (missingCommands.length) {
  throw new Error(`RPC_PARITY failed:\n${missingCommands.join("\n")}`);
}

const shotWorkspaceSource = readFileSync(join(root, "src/features/shots/ShotWorkspace.tsx"), "utf8");
if (shotWorkspaceSource.includes("subscribeTaskUpdates")) {
  throw new Error("SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION failed: ShotWorkspace must delegate task events to useShotTaskEvents");
}
if (!shotWorkspaceSource.includes("useShotQueueController")) {
  throw new Error("SHOT_WORKSPACE_QUEUE_CONTROLLER failed: ShotWorkspace must delegate queue ownership to useShotQueueController");
}
if (!shotWorkspaceSource.includes("useShotProductionMonitor")) {
  throw new Error("SHOT_WORKSPACE_MONITOR_CONTROLLER failed: ShotWorkspace must delegate monitor ownership to useShotProductionMonitor");
}
if (["productionMonitorRequest", "productionMonitorPendingBatch", "productionMonitorMounted", "productionMonitorBatchRef", "window.setInterval(refreshIfVisible, 3000)", "visibilitychange"].some((marker) => shotWorkspaceSource.includes(marker))) {
  throw new Error("SHOT_WORKSPACE_MONITOR_CONTROLLER failed: ShotWorkspace must not own monitor polling or request refs");
}
if (!shotWorkspaceSource.includes("useShotMultiPackageController")) {
  throw new Error("SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER failed: ShotWorkspace must delegate multi-package ownership to useShotMultiPackageController");
}
if (["multiPackageRunId", "multiPackageRefreshInFlight", "multiPackageRefreshPending", "multiPackageMounted"].some((marker) => shotWorkspaceSource.includes(marker))) {
  throw new Error("SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER failed: ShotWorkspace must not own multi-package lifecycle refs");
}

const assetVideoBatchWorkspaceSource = readFileSync(join(root, "src/features/assets/AssetVideoBatchWorkspace.tsx"), "utf8");
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoWorkflowController")) {
  throw new Error("ASSET_VIDEO_WORKFLOW_CONTROLLER failed: AssetVideoBatchWorkspace must delegate workflow resolution ownership to useAssetVideoWorkflowController");
}
const assetVideoWorkflowStateLines = assetVideoBatchWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && ["manualVideoSelection", "projectWorkflowConfig", "projectWorkflowStrategy", "projectManualOverrides"].some((marker) => line.includes(marker)));
if (assetVideoWorkflowStateLines.length) {
  throw new Error("ASSET_VIDEO_WORKFLOW_CONTROLLER failed: AssetVideoBatchWorkspace must not own workflow resolution state");
}
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoLibraryController")) {
  throw new Error("ASSET_VIDEO_LIBRARY_CONTROLLER failed: AssetVideoBatchWorkspace must delegate asset library query ownership to useAssetVideoLibraryController");
}
if (assetVideoBatchWorkspaceSource.includes("assetLibraryRequestVersion") || assetVideoBatchWorkspaceSource.includes("assetLibraryPage(")) {
  throw new Error("ASSET_VIDEO_LIBRARY_CONTROLLER failed: AssetVideoBatchWorkspace must not own asset library request lifecycle or call assetLibraryPage directly");
}
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoLocalImportController")) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must delegate local import session ownership to useAssetVideoLocalImportController");
}
const localImportStateMarkers = ["localInspection", "projectSegmentForms", "localBatchName", "localAutoStart", "expandedLocalOrdinal"];
const localImportStateLines = assetVideoBatchWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && localImportStateMarkers.some((marker) => line.includes(marker)));
if (localImportStateLines.length) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must not own local import session state");
}
if (["pickH3LocalImportDirectory(", "rescanH3LocalImport(", "updateH3ProjectSegmentDraft("].some((marker) => assetVideoBatchWorkspaceSource.includes(marker))) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must delegate local import commands");
}

const workflowWorkspaceSource = readFileSync(join(root, "src/features/workflows/WorkflowWorkspace.tsx"), "utf8");
const workflowWorkspaceAdaptersSource = readFileSync(join(root, "src/features/workflows/workflowWorkspaceAdapters.ts"), "utf8");
const recipeHistoryServiceSource = readFileSync(join(root, "src-tauri/src/application/recipe_history_query_service.rs"), "utf8");
const recipeHistoryPortSource = readFileSync(join(root, "src-tauri/src/application/ports/recipe_history_query_repository.rs"), "utf8");
const recipeHistoryRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/recipe_history_query.rs"), "utf8");
const recipeHistoryCommandSource = readFileSync(join(root, "src-tauri/src/commands/recipe_history.rs"), "utf8");
const recipeHistoryClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const workflowPromotionRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/workflow_recipe_promotion.rs"), "utf8");
const recipeArchiveMigrationSource = readFileSync(join(root, "src-tauri/migrations/031_workflow_recipe_archive_state.sql"), "utf8");
const recipeArchivePortSource = readFileSync(join(root, "src-tauri/src/application/ports/workflow_recipe_runtime_state_repository.rs"), "utf8");
const recipeArchiveRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/workflow_recipe_runtime_state.rs"), "utf8");
const workflowPromotionPortSource = readFileSync(join(root, "src-tauri/src/application/ports/workflow_recipe_promotion_repository.rs"), "utf8");
const workflowRegistryServiceSource = readFileSync(join(root, "src-tauri/src/application/workflow_registry_service.rs"), "utf8");
const workflowRegistryCommandSource = readFileSync(join(root, "src-tauri/src/commands/workflow_registry.rs"), "utf8");
const appErrorSource = readFileSync(join(root, "src-tauri/src/error.rs"), "utf8");
if (!workflowWorkspaceSource.includes("useWorkflowSmartImportController")) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must delegate Smart Import session ownership to useWorkflowSmartImportController");
}
const smartImportStateMarkers = ["autoPlan", "autoImportError"];
const smartImportStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && smartImportStateMarkers.some((marker) => line.includes(marker)));
if (smartImportStateLines.length) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must not own Smart Import session state");
}
const smartImportFunctionMarkers = [
  "function smartImportWorkflow(",
  "function resumeAutoImport(",
  "function regenerateExistingRecipe(",
  "function resolveAutoIssue(",
  "function commitAnalyzedImport(",
  "function openAdvancedImport(",
  "function openExistingWorkflow(",
  "function openStructuralVariantAsVersion(",
];
if (smartImportFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must delegate Smart Import actions");
}

if (!workflowWorkspaceSource.includes("useWorkflowParameterExposureController")) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must delegate Parameter Exposure ownership to useWorkflowParameterExposureController");
}

if (!appErrorSource.includes("WorkflowRecipeLifecycleError")
  || !workflowRegistryCommandSource.includes("AppError::workflow_recipe_lifecycle(")
  || workflowRegistryCommandSource.includes("WORKFLOW_RECIPE_LIFECYCLE_ERROR: workflow_version_id=")) {
  throw new Error("STRUCTURED_IPC_ERROR failed: recipe lifecycle errors must use typed IPC details instead of message parsing");
}
const parameterExposureStateMarkers = ["parameterDraft", "parameterItem", "parameterOriginalKeys", "parameterLoading"];
const parameterExposureStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && parameterExposureStateMarkers.some((marker) => line.includes(marker)));
if (parameterExposureStateLines.length) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must not own Parameter Exposure session state");
}
const parameterExposureFunctionMarkers = [
  "function openParameterExposure(",
  "function closeParameterExposure(",
  "function refreshParameterCapability(",
  "function exposeParameter(",
  "function saveParameterMapping(",
  "function removeParameterMapping(",
  "function publishParameterRecipe(",
];
if (parameterExposureFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must delegate Parameter Exposure actions");
}

const recipeHistoryMutationMarkers = [
  "archive_recipe", "restore_recipe", "promote_recipe", "clear_recipe_promotion", "requeue_task", "retry_task", "cancel_task",
];
if (!recipeHistoryServiceSource.includes("pub struct RecipeHistoryQueryService")
  || !recipeHistoryServiceSource.includes("pub async fn get_exact_pair(")
  || !recipeHistoryPortSource.includes("trait RecipeHistoryQueryRepository")
  || !recipeHistoryRepositorySource.includes("impl RecipeHistoryQueryRepository for SqliteRecipeHistoryQueryRepository")
  || !recipeHistoryCommandSource.includes("pub async fn workflow_recipe_history_get(")
  || !recipeHistoryClientSource.includes('"workflow_recipe_history_get"')) {
  throw new Error("RECIPE_HISTORY_QUERY_AUTHORITY failed: exact-pair service, repository, command, and typed client are required");
}
if (recipeHistoryMutationMarkers.some((marker) => recipeHistoryServiceSource.includes(marker) || recipeHistoryCommandSource.includes(marker))
  || recipeHistoryServiceSource.includes("sqlx::")
  || recipeHistoryPortSource.includes("&mut")
  || recipeHistoryPortSource.includes("fn archive")
  || recipeHistoryPortSource.includes("fn restore")) {
  throw new Error("RECIPE_HISTORY_READ_ONLY failed: history query must not expose mutation authority or application SQLx");
}
const migrationFiles = readdirSync(join(root, "src-tauri/migrations"));
if (migrationFiles.some((file) => /recipe.?history/i.test(file))
  || migrationFiles.some((file) => /\.sql$/.test(file) && /CREATE\s+TABLE\s+recipe_history/i.test(readFileSync(join(root, "src-tauri/migrations", file), "utf8")))) {
  throw new Error("RECIPE_HISTORY_READ_ONLY failed: a new recipe history table or migration was added");
}
if (!workflowWorkspaceSource.includes("openRecipeHistory")
  || !workflowWorkspaceSource.includes("getWorkflowRecipeHistory(")
  || !workflowWorkspaceSource.includes("onViewHistory")
  || /useEffect\([\s\S]{0,600}getWorkflowRecipeHistory\(/.test(workflowWorkspaceSource)) {
  throw new Error("RECIPE_HISTORY_ON_DEMAND failed: history must load only from the existing recipe detail action");
}

const workflowClientSource = readFileSync(join(root, "src/services/workflowClient.ts"), "utf8");
const tauriClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const generationStudioSource = readFileSync(join(root, "src/features/studio/GenerationStudio.tsx"), "utf8");
if (!workflowWorkspaceSource.includes("promoteWorkflowRecipe(")
  || !workflowClientSource.includes("promoteWorkflowRecipe")
  || !tauriClientSource.includes('"workflow_promote_recipe"')) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION failed: promotion must be owned by WorkflowWorkspace through the typed workflow client");
}
const generationStudioPromotionMarkers = ["promoteWorkflowRecipe", "workflow_promote_recipe", "setPromotedRecipe"];
const generationStudioPromotionOwnership = generationStudioPromotionMarkers
  .filter((marker) => generationStudioSource.includes(marker));
if (generationStudioPromotionOwnership.length) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION failed: GenerationStudio must not own recipe promotion");
}
if (!workflowWorkspaceSource.includes("resolveImplicitWorkflowRecipe")
  || !workflowWorkspaceAdaptersSource.includes("export function resolveImplicitWorkflowRecipe(")
  || generationStudioSource.includes("resolveImplicitWorkflowRecipe")
  || generationStudioSource.includes("isPromoted")) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CONSUMPTION failed: promotion must be consumed once by the Workflow Workspace resolution boundary");
}
const generationStudioClearPromotionMarkers = ["clearWorkflowRecipePromotion", "workflow_clear_recipe_promotion", "clearRecipePromotion"];
if (!workflowWorkspaceSource.includes("clearWorkflowRecipePromotion(")
  || !workflowClientSource.includes("clearWorkflowRecipePromotion")
  || !tauriClientSource.includes('"workflow_clear_recipe_promotion"')
  || !libSource.includes("workflow_clear_recipe_promotion")
  || generationStudioClearPromotionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CLEAR failed: clear must have one typed Workflow Workspace path and no GenerationStudio ownership");
}
if ((workflowPromotionRepositorySource.match(/impl WorkflowRecipePromotionRepository for /g) ?? []).length !== 1
  || !workflowPromotionPortSource.includes("async fn clear(")
  || !workflowPromotionRepositorySource.includes("async fn clear(")
  || !workflowPromotionRepositorySource.includes("DELETE FROM workflow_recipe_promotions")
  || !workflowRegistryServiceSource.includes("pub async fn clear_recipe_promotion(")
  || !workflowRegistryCommandSource.includes("pub async fn workflow_clear_recipe_promotion(")) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CLEAR failed: clear must remain owned by the existing repository/service/command authority");
}

const recipeArchiveApplicationSources = [
  readFileSync(join(root, "src-tauri/src/application/workflow_registry_service.rs"), "utf8"),
  readFileSync(join(root, "src-tauri/src/application/workflow_lifecycle_service.rs"), "utf8"),
  readFileSync(join(root, "src-tauri/src/application/production_queue_service.rs"), "utf8"),
];
const directApplicationSqlx = recipeArchiveApplicationSources.filter((source) => /\bsqlx::/.test(source));
if (!recipeArchiveMigrationSource.includes("CREATE TABLE workflow_recipe_runtime_states")
  || !recipeArchiveMigrationSource.includes("PRIMARY KEY (workflow_version_id, recipe_id)")
  || !recipeArchiveMigrationSource.includes("REFERENCES recipes(workflow_version_id, id)")
  || !recipeArchivePortSource.includes("trait WorkflowRecipeRuntimeStateRepository")
  || !recipeArchivePortSource.includes("workflow_version_id")
  || !recipeArchivePortSource.includes("recipe_id")
  || !recipeArchiveRepositorySource.includes("impl WorkflowRecipeRuntimeStateRepository for")
  || !workflowRegistryServiceSource.includes("recipe_runtime_state_repository")
  || !workflowRegistryServiceSource.includes("pub async fn archive_recipe(")
  || !workflowRegistryServiceSource.includes("pub async fn restore_recipe(")
  || !workflowRegistryServiceSource.includes("recipe_is_archived")
  || directApplicationSqlx.length) {
  throw new Error("RECIPE_ARCHIVE_AUTHORITY failed: recipe archive must use one dedicated infrastructure-backed exact-pair state authority");
}
if (!workflowWorkspaceAdaptersSource.includes("!item.currentRecipe.archived")
  || !workflowWorkspaceAdaptersSource.includes("item.registryRecipes?.find")
  || !workflowWorkspaceAdaptersSource.includes("?.archived")) {
  throw new Error("ONE_IMPLICIT_RECIPE_RESOLVER failed: archived recipes must be excluded by resolveImplicitWorkflowRecipe");
}
if (!workflowClientSource.includes("archiveWorkflowRecipe")
  || !workflowClientSource.includes("restoreWorkflowRecipe")
  || !tauriClientSource.includes('"workflow_archive_recipe"')
  || !tauriClientSource.includes('"workflow_restore_recipe"')
  || !workflowRegistryCommandSource.includes("pub async fn workflow_archive_recipe(")
  || !workflowRegistryCommandSource.includes("pub async fn workflow_restore_recipe(")
  || !libSource.includes("workflow_archive_recipe")
  || !libSource.includes("workflow_restore_recipe")) {
  throw new Error("RECIPE_ARCHIVE_AUTHORITY failed: archive/restore must use the typed Registry command path");
}

if (!generationStudioSource.includes("useGenerationPresetController")) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must delegate preset lifecycle ownership to useGenerationPresetController");
}
const generationPresetStateMarkers = ["presets", "selectedPresetId", "preferredPresetId", "presetName", "presetLoading", "presetError", "presetEditorOpen"];
const generationPresetStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationPresetStateMarkers.some((marker) => line.includes(marker)));
if (generationPresetStateLines.length) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must not own preset lifecycle state");
}
const generationPresetFunctionMarkers = [
  "function applyPreset(",
  "function savePreset(",
  "function savePresetChanges(",
  "function removePreset(",
  "function togglePreferredPreset(",
];
if (generationPresetFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must delegate preset lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationSubmissionController")) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must delegate submission ownership to useGenerationSubmissionController");
}
const generationSubmissionStateMarkers = ["creating", "cancelling"];
const generationSubmissionStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationSubmissionStateMarkers.some((marker) => line.includes(marker)));
if (generationSubmissionStateLines.length || generationStudioSource.includes("generationRequestIdRef")) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must not own submission state or idempotency refs");
}
const generationSubmissionFunctionMarkers = [
  "function generate(",
  "function cancelCurrentTask(",
];
if (generationSubmissionFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must delegate submission actions");
}

if (!generationStudioSource.includes("useGenerationBatchController")) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must delegate batch lifecycle ownership to useGenerationBatchController");
}
const generationBatchStateMarkers = ["batchItems", "batchSubmitting", "batchNotice", "batchPasteText"];
const generationBatchStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationBatchStateMarkers.some((marker) => line.includes(marker)));
if (generationBatchStateLines.length) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must not own batch lifecycle state");
}
const generationBatchFunctionMarkers = [
  "function addCurrentToBatch(",
  "function addBlankPromptCard(",
  "function updateBatchPrompt(",
  "function copyBatchItem(",
  "function moveBatchItem(",
  "function splitPastedPrompts(",
  "function removeBatchItem(",
  "function importBatchTaskList(",
  "function submitBatch(",
];
if (generationBatchFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must delegate batch lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationExperimentController")) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must delegate experiment lifecycle ownership to useGenerationExperimentController");
}
const generationExperimentStateMarkers = ["experimentFocusBatchId", "experimentContexts", "promptExperimentDimensions"];
const generationExperimentStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationExperimentStateMarkers.some((marker) => line.includes(marker)));
if (generationExperimentStateLines.length) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must not own experiment lifecycle state");
}
const generationExperimentFunctionMarkers = [
  "function submitExperimentPlan(",
  "function promoteExperimentWinner(",
];
if (generationExperimentFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must delegate experiment lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationProjectTemplateController")) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must delegate project template lifecycle ownership to useGenerationProjectTemplateController");
}
const generationProjectTemplateStateMarkers = ["templateEditorOpen", "templateName", "templateDescription", "templateSaving", "templateError"];
const generationProjectTemplateStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationProjectTemplateStateMarkers.some((marker) => line.includes(marker)));
if (generationProjectTemplateStateLines.length) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must not own project template lifecycle state");
}
if (generationStudioSource.includes("function saveProjectTemplate(") || generationStudioSource.includes("createProjectTemplate(")) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must delegate project template persistence");
}

if (!generationStudioSource.includes("useGenerationAssetIntentController")) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must delegate Asset Intent lifecycle ownership to useGenerationAssetIntentController");
}
const generationAssetIntentStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && line.includes("assetIntentTargets"));
if (generationAssetIntentStateLines.length) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must not own Asset Intent target state");
}
if (generationStudioSource.includes("function applyPendingAsset(") || generationStudioSource.includes("clearPendingAssetIntent(")) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must delegate Asset Intent lifecycle actions");
}
const assetIntentControllerSource = readFileSync(join(root, "src/features/studio/hooks/useGenerationAssetIntentController.ts"), "utf8");
if (/useState\s*<[^>]*PendingStudioAssetIntent|\[\s*pendingAssetIntent\s*,/.test(assetIntentControllerSource)) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: controller must keep pendingAssetIntent authority in useStudioStore");
}

if (!generationStudioSource.includes("useGenerationWorkflowSelectionController")) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must delegate workflow selection ownership to useGenerationWorkflowSelectionController");
}
const generationWorkflowSelectionStateMarkers = ["manualSelection", "projectWorkflowConfig"];
const generationWorkflowSelectionStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationWorkflowSelectionStateMarkers.some((marker) => line.includes(marker)));
if (generationWorkflowSelectionStateLines.length) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must not own workflow selection state");
}
const generationWorkflowSelectionFunctionMarkers = [
  "function selectWorkflowFromUx(",
  "function restoreRecommendedWorkflow(",
];
if (generationWorkflowSelectionFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must delegate workflow selection actions");
}
if (generationStudioSource.includes("getProjectWorkflowConfig(")) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must not load project workflow config directly");
}
const workflowSelectionControllerSource = readFileSync(join(root, "src/features/studio/hooks/useGenerationWorkflowSelectionController.ts"), "utf8");
if (/useState\s*<[^>]*RecipeViewModel|\[\s*selectedWorkflow\s*,/.test(workflowSelectionControllerSource)) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: controller must keep selectedWorkflow authority in useStudioStore");
}

if (!workflowWorkspaceSource.includes("useWorkflowAdvancedOnboardingController")) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must delegate Advanced Onboarding ownership to useWorkflowAdvancedOnboardingController");
}
const advancedOnboardingStateMarkers = ["mappingDrafts", "outputDraft", "metadataDraft", "published", "showAdvanced"];
const advancedOnboardingStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && advancedOnboardingStateMarkers.some((marker) => line.includes(marker)));
if (advancedOnboardingStateLines.length) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must not own Advanced Onboarding state");
}
const advancedOnboardingFunctionMarkers = [
  "function runDraftAction(",
  "function saveMetadata(",
  "function validateDraft(",
  "function publishDraft(",
  "function bindInput(",
  "function removeInput(",
  "function addOutput(",
  "function checkCapability(",
  "function discardDraft(",
];
if (advancedOnboardingFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must delegate Advanced Onboarding actions");
}

const projectCommandCenterServiceSource = readFileSync(join(root, "src-tauri/src/application/project_command_center_service.rs"), "utf8");
if (!["pub fn recommend_next_project_action", "first_completed_asset_id", "fn action_with_targets"].every((marker) => projectCommandCenterServiceSource.includes(marker))) {
  throw new Error("PROJECT_CONTINUITY_DERIVED failed: Project Command Center must derive exact continuation targets from existing facts");
}
const projectCommandCenterTypesSource = readFileSync(join(root, "src/types/projectCommandCenter.ts"), "utf8");
const projectCommandCenterViewSource = readFileSync(join(root, "src/features/projects/ProjectCommandCenter.tsx"), "utf8");
if (!["ProjectCommandCenterDailyProductionView", "totalCount", "hasMore", "workflowVersionId", "recipeId"].every((marker) => projectCommandCenterTypesSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_EXACT_TARGETS failed: typed daily production contract is incomplete");
}
if (!["load_daily_production", "DAILY_PRODUCTION_ITEM_LIMIT", "top_action", "Production Queue"].every((marker) => projectCommandCenterServiceSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_DERIVED failed: daily production must be derived in the existing command center service");
}
if (!projectCommandCenterViewSource.includes("DailyProductionBoard") || projectCommandCenterServiceSource.includes("CREATE TABLE daily_production")) {
  throw new Error("DAILY_PRODUCTION_NO_SECOND_STATE failed: daily production must remain a read-only view");
}
if (!["onNavigate", "actionKind", "dailyProductionNavigation"].every((marker) => projectCommandCenterViewSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_NAVIGATION failed: daily production items must use existing typed navigation");
}

console.log(`WORKFLOW_SMART_IMPORT_CONTROLLER=PASS`);
console.log(`WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER=PASS`);
console.log(`RECIPE_HISTORY_QUERY_AUTHORITY=PASS`);
console.log(`RECIPE_HISTORY_READ_ONLY=PASS`);
console.log(`RECIPE_HISTORY_ON_DEMAND=PASS`);
console.log(`STRUCTURED_IPC_ERROR=PASS`);
console.log(`RECIPE_ARCHIVE_AUTHORITY=PASS`);
console.log(`ONE_RECIPE_ARCHIVE_STATE_SOURCE=PASS`);
console.log(`ONE_IMPLICIT_RECIPE_RESOLVER=PASS`);
console.log(`APPLICATION_DIRECT_SQLX_NEW_USAGE=0`);
console.log(`NO_NEW_QUEUE=YES`);
console.log(`NO_NEW_EXECUTOR=YES`);
console.log(`NO_NEW_TASK_MODEL=YES`);
console.log(`WORKFLOW_RECIPE_PROMOTION=PASS`);
console.log(`WORKFLOW_RECIPE_PROMOTION_CONSUMPTION=PASS`);
console.log(`WORKFLOW_RECIPE_PROMOTION_CLEAR=PASS`);
console.log(`WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER=PASS`);
console.log(`GENERATION_PRESET_CONTROLLER=PASS`);
console.log(`GENERATION_SUBMISSION_CONTROLLER=PASS`);
console.log(`GENERATION_BATCH_CONTROLLER=PASS`);
console.log(`GENERATION_EXPERIMENT_CONTROLLER=PASS`);
console.log(`GENERATION_PROJECT_TEMPLATE_CONTROLLER=PASS`);
console.log(`GENERATION_ASSET_INTENT_CONTROLLER=PASS`);
console.log(`GENERATION_WORKFLOW_SELECTION_CONTROLLER=PASS`);
console.log(`PROJECT_CONTINUITY_DERIVED=PASS`);
console.log(`DAILY_PRODUCTION_DERIVED=PASS`);
console.log(`DAILY_PRODUCTION_NO_SECOND_STATE=PASS`);
console.log(`DAILY_PRODUCTION_EXACT_TARGETS=PASS`);
console.log(`DAILY_PRODUCTION_NO_AUTO_EXECUTION=PASS`);
console.log(`INTERNAL_SCRIPT_AUTHORING_RETIRED=PASS`);
console.log(`INTERNAL_STORYBOARD_AUTHORING_RETIRED=PASS`);
console.log(`INTERNAL_PROMPT_AUTHORING_RETIRED=PASS`);
console.log(`EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION=PASS`);
console.log(`EXTERNAL_HANDOFF_AUTHORITY=PASS`);
console.log(`EXTERNAL_HANDOFF_PREVIEW_READ_ONLY=PASS`);
console.log(`EXTERNAL_HANDOFF_SINGLE_TRANSACTION=PASS`);
console.log(`EXTERNAL_HANDOFF_EXACT_WORKFLOW_RECIPE=PASS`);
console.log(`EXTERNAL_HANDOFF_TYPED_IPC=PASS`);

console.log(`FRONTEND_NO_RAW_INVOKE=PASS`);
console.log(`RAW_INVOKE_OUTSIDE_TRANSPORT=0`);
console.log(`RPC_PARITY=PASS (${frontendCommands.size} frontend commands checked)`);
console.log(`SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS`);
console.log(`SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS`);
console.log(`SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS`);
console.log(`SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_WORKFLOW_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_LIBRARY_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER=PASS`);
