import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowImportCommitAction,
  WorkflowOnboardingDraftView,
} from "../../types/workflowOnboarding";
import { WorkflowSmartImport } from "./WorkflowSmartImport";
import type { WorkflowImportErrorView } from "./WorkflowImportIssues";

export interface WorkflowImportControllerProps {
  plan?: WorkflowAutoOnboardingPlanView;
  draft?: WorkflowOnboardingDraftView;
  loading: boolean;
  projectId?: string;
  importError?: WorkflowImportErrorView;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onOpenExistingVersion?: () => void;
  onRegenerateRecipe?: () => void;
  onRestoreExisting?: () => void;
  onCommitImport: (action: WorkflowImportCommitAction) => void;
  onOpenStudio?: (workflowId: string, recipeId: string) => void;
  onUseInProject?: (workflowId: string, recipeId: string) => void;
  onReturnToList: () => void;
  onRetry: () => void;
}

/**
 * Formal import control plane: analyze is read-only and commit is explicit.
 * The advanced editor remains available through its dedicated draft APIs.
 */
export function WorkflowImportController(props: WorkflowImportControllerProps) {
  return <WorkflowSmartImport {...props} onCancel={props.onReturnToList} />;
}
