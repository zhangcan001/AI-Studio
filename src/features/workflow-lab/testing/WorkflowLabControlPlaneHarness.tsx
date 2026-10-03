import { WorkflowLabSurface } from "../WorkflowLabSurface";
import { useWorkflowLabController, type WorkflowWorkspaceProps } from "../../workflows/useWorkflowLabController";
/** Test-only composition of the production Lab control plane; never a runtime entry. */
export function WorkflowLabControlPlaneHarness(props: WorkflowWorkspaceProps) {
  return <WorkflowLabSurface controller={useWorkflowLabController(props)} />;
}
