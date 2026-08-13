import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
} from "../../types/workflowOnboarding";
import { WorkflowImportIssues } from "./WorkflowImportIssues";
import { WorkflowImportResult } from "./WorkflowImportResult";

interface Props {
  plan?: WorkflowAutoOnboardingPlanView;
  loading: boolean;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onRestoreExisting?: () => void;
  onOpenStudio?: (workflowId: string, recipeId: string) => void;
}

export function WorkflowSmartImport({ plan, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onRestoreExisting, onOpenStudio }: Props) {
  if (!plan) return null;
  if (plan.state === "AUTO_PUBLISHED") {
    return <WorkflowImportResult plan={plan} onOpenAdvanced={onOpenAdvanced} onOpenStudio={onOpenStudio} />;
  }
  return (
    <WorkflowImportIssues
      plan={plan}
      loading={loading}
      onResolve={onResolve}
      onResume={onResume}
      onOpenAdvanced={onOpenAdvanced}
      onOpenExisting={onOpenExisting}
      onRestoreExisting={onRestoreExisting}
    />
  );
}
