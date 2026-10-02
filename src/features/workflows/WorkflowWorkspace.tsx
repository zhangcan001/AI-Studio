import { useWorkflowLabController, type WorkflowWorkspaceProps } from "./useWorkflowLabController";
import { WorkflowLabSurface } from "../workflow-lab/WorkflowLabSurface";
export type { WorkflowWorkspaceProps } from "./useWorkflowLabController";
export { SeedModeSelect } from "../workflow-lab/labViewHelpers";
export { latestCatalogRecipeForWorkflowItem, normalizeWorkspaceItem, resolveImplicitWorkflowRecipe } from "./workflowWorkspaceAdapters";
export { isExposableWorkflowInput } from "./workflowParameterExposureModel";
export { createDefaultOutputDraft } from "./hooks/useWorkflowAdvancedOnboardingController";
/** Legacy entry remains available without data conversion. Shares existing authority. */
export function WorkflowWorkspace(props: WorkflowWorkspaceProps) {
 return <WorkflowLabSurface controller={useWorkflowLabController(props)} />;
}
