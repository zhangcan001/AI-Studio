import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(import.meta.url), "..", "..");
const ipcTransportPath = resolve(root, "src/services/ipc.ts");
const productionFrontendFiles = [];

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

const generationStudioSource = readFileSync(join(root, "src/features/studio/GenerationStudio.tsx"), "utf8");
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

console.log(`WORKFLOW_SMART_IMPORT_CONTROLLER=PASS`);
console.log(`WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER=PASS`);
console.log(`WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER=PASS`);
console.log(`GENERATION_PRESET_CONTROLLER=PASS`);
console.log(`GENERATION_SUBMISSION_CONTROLLER=PASS`);
console.log(`GENERATION_BATCH_CONTROLLER=PASS`);

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
