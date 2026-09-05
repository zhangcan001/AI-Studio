import { invoke } from "@tauri-apps/api/core";
import * as transport from "./tauriClient";
import type { WorkflowProductionWorkspaceResponse } from "../types/workflowOnboarding";
import type {
  WorkflowWorkspaceQueryItem,
  WorkflowWorkspaceQueryMode,
  WorkflowWorkspaceQueryResponse,
  WorkflowWorkspaceRuntimeView,
} from "../features/workflows/workflowWorkspaceAdapters";

/** The formal workspace is the single registry + runtime read model. */
export type { WorkflowWorkspaceQueryMode, WorkflowWorkspaceQueryResponse } from "../features/workflows/workflowWorkspaceAdapters";

export function queryWorkflowWorkspace(mode: WorkflowWorkspaceQueryMode): Promise<WorkflowWorkspaceQueryResponse> {
  return invoke<WorkflowWorkspaceQueryResponse>("workflow_workspace_query", { mode }).catch(async (error: unknown) => {
    // Existing component UATs run without a Tauri command registry. Keep their
    // old transport mock usable without exposing this compatibility path in a
    // production build or in the formal WorkflowWorkspace component.
    if (import.meta.env.MODE !== "test") throw error;
    return adaptLegacyWorkspaceForTest(await transport.listWorkflowProductionWorkspace());
  });
}

/**
 * Test-only shape bridge for pre-DEV-084 component tests. Production code must
 * provide these fields from workflow_workspace_query itself.
 */
function adaptLegacyWorkspaceForTest(response: WorkflowProductionWorkspaceResponse): WorkflowWorkspaceQueryResponse {
  return {
    items: response.items.map((item) => {
      const workflowId = item.workflowId ?? item.packageName;
      const workflowVersionId = item.workflowVersionId ?? `${workflowId}:version`;
      const recipes = item.recipes.map((recipe) => ({
        workflowVersionId,
        recipeId: recipe.recipeId,
        version: recipe.version,
        schemaVersion: 1,
        recipeSha256: item.recipeSha256 ?? "",
        packageName: item.packageName,
      }));
      const currentRecipe = recipes[recipes.length - 1];
      const runtime: WorkflowWorkspaceRuntimeView = {
        workflowId,
        workflowVersionId,
        recipeId: currentRecipe?.recipeId ?? "",
        name: item.name ?? item.packageName,
        category: item.category ?? "",
        mode: item.mode ?? "",
        workflowVersion: item.workflowVersion ?? "",
        recipeVersion: currentRecipe?.version ?? "",
        workflowSha256: item.workflowSha256 ?? "",
        recipeSha256: item.recipeSha256 ?? "",
        artifactId: item.packageName,
        artifactSourceKind: item.source,
        packageName: item.packageName,
        packageSourcePath: undefined,
        artifactStatus: item.packageStatus,
        packageStatus: item.packageStatus,
        libraryState: item.archived ? "REMOVED" : "ACTIVE",
        enabled: item.enabled,
        archived: item.archived,
        archivedAt: item.archivedAt,
        capability: item.capability,
        capabilityIssues: item.capabilityIssues,
        readiness: item.readiness,
        readinessReasons: item.readinessReasons,
        diagnostics: item.diagnostics,
        nodeCount: item.nodeCount,
        liveVerifiedAt: item.liveVerifiedAt,
        hasSuccessfulRun: item.hasSuccessfulRun,
        latestSuccessAt: item.latestSuccessAt,
        latestFailureAt: item.latestFailureAt,
        activeTasks: item.activeTasks,
        totalTasks: item.totalTasks,
      };
      return {
        legacyTestAdapter: true,
        registry: {
          workflowId,
          name: item.name ?? item.packageName,
          sourceKind: item.builtin || item.source?.trim().toUpperCase() === "PRODUCT" ? "PRODUCT" : "USER",
          libraryState: item.archived ? "REMOVED" : "ACTIVE",
          currentVersionId: workflowVersionId,
          currentVersion: {
            workflowVersionId,
            workflowId,
            version: item.workflowVersion ?? "",
            workflowSha256: item.workflowSha256 ?? "",
            isCurrent: true,
            enabled: item.enabled,
            archived: item.archived,
            recipes,
          },
          currentRecipe,
          versions: [{
          workflowVersionId,
          workflowId,
          version: item.workflowVersion ?? "",
          workflowSha256: item.workflowSha256 ?? "",
          isCurrent: true,
          enabled: item.enabled,
          archived: item.archived,
          recipes,
          }],
          recipes,
          projectUsageCount: 0,
          historyCount: item.totalTasks,
        },
        runtime: item.recipes.length ? [runtime] : [],
      };
    }) as WorkflowWorkspaceQueryItem[],
    staging: response.staging,
  };
}

// Workflow onboarding and lifecycle calls are exposed from this domain
// client. Keeping the existing transport bindings preserves shared error
// handling and lets callers migrate without changing the native commands.
export {
  analyzeWorkflowImport,
  checkOnboardingCapability,
  cleanWorkflowStaging,
  commitWorkflowImport,
  compareWorkflowVersions,
  discardOnboarding,
  deleteWorkflow,
  deleteWorkflowVersion,
  duplicateWorkflowRecipe,
  exportWorkflowPackage,
  getOnboardingDraft,
  importWorkflowPackageBackup,
  inspectWorkflowDeletion,
  repairBuiltinWorkflowPackage,
  pickApiWorkflow,
  publishOnboarding,
  recheckWorkflowCapability,
  recheckAllWorkflowCapabilities,
  removeWorkflow,
  renameWorkflow,
  rerecognizeWorkflow,
  removeOnboardingInputMapping,
  restoreWorkflowVersion,
  restoreWorkflow,
  purgeWorkflow,
  setWorkflowCurrentVersion,
  setWorkflowEnabled,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
  validateOnboarding,
} from "./tauriClient";
