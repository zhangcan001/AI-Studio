/** Approved Advanced seam. Re-exports existing typed authority calls only. No persistence, inference or execution authority. */
/** Stateless transport locator only. Mirrors Product's versioned canonical encoding;
 * Product authorities still validate project scope, lifecycle and the exact pair. */
export function labCreationSelection(workflowVersionId: string, recipeId: string): string {
  if (!workflowVersionId.trim() || !recipeId.trim()) throw new Error("请选择确切的工作流版本和配方。");
  const bytes = new TextEncoder().encode(JSON.stringify([workflowVersionId, recipeId]));
  if (bytes.length > 4096) throw new Error("生成器标识过长。");
  return "generator:v1:" + Array.from(bytes, b => b.toString(16).padStart(2, "0")).join("");
}
export {
  analyzeWorkflowImport,
  archiveWorkflowRecipe,
  checkOnboardingCapability,
  cleanWorkflowStaging,
  clearWorkflowRecipePromotion,
  commitWorkflowImport,
  compareWorkflowVersions,
  deleteWorkflow,
  deleteWorkflowVersionOf,
  discardOnboarding,
  duplicateWorkflowRecipe,
  exportWorkflowPackage,
  getOnboardingDraft,
  getSavedWorkflowVersionDetails,
  getWorkflowRecipeHistory,
  importWorkflowPackageBackup,
  inspectWorkflowDeletion,
  inspectWorkflowPurge,
  pickApiWorkflow,
  promoteWorkflowRecipe,
  purgeWorkflow,
  queryWorkflowWorkspace,
  reanalyzeWorkflowImport,
  recheckAllWorkflowCapabilities,
  recheckWorkflowCapability,
  removeOnboardingInputMapping,
  removeWorkflow,
  renameWorkflow,
  repairBuiltinWorkflowPackage,
  rerecognizeWorkflow,
  restoreWorkflow,
  restoreWorkflowRecipe,
  restoreWorkflowVersion,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
  setWorkflowCurrentVersion,
  setWorkflowEnabled,
  validateOnboarding
} from "./workflowClient";
export {
  repairJobsStatus,
  cloneWorkflowBenchmark,
  createWorkflowBenchmark,
  deleteWorkflowBenchmark,
  getAsset,
  getWorkflowBenchmark,
  listPresets,
  listWorkflowBenchmarks,
  previewWorkflowBenchmark,
  queueWorkflowBenchmark,
  saveWorkflowBenchmarkQuality,
  setWorkflowBenchmarkRecommendation,
  setWorkflowBenchmarkWinner,

  cancelPendingProductionQueue,
  createWorkflowExecution,
  createWorkflowExecutionBatch,
  getProductionQueue,
  getProjectWorkflowConfig,
  getTaskDetail,
  getWorkflowExecutionSummary,
  listModelVersions,
  listModels,
  listRuntimeProfiles,
  preflightWorkflowExecution,
  startProductionQueue,
  submitGeneration
} from "./tauriClient";
