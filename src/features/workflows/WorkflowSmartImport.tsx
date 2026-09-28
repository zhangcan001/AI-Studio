import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowImportCommitAction,
  WorkflowImportFormat,
  WorkflowOnboardingDraftView,
} from "../../types/workflowOnboarding";
import { WorkflowImportFormatIssue, WorkflowImportIssues, type WorkflowImportErrorView } from "./WorkflowImportIssues";
import { WorkflowImportResult } from "./WorkflowImportResult";

interface Props {
  plan?: WorkflowAutoOnboardingPlanView;
  loading: boolean;
  onResolve: (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => void;
  onResume: () => void;
  onOpenAdvanced: () => void;
  onOpenExisting: () => void;
  onOpenExistingVersion?: () => void;
  onRegenerateRecipe?: () => void;
  onRestoreExisting?: () => void;
  onCommitImport?: (action: WorkflowImportCommitAction) => void;
  onSaveReviewMetadata?: (metadata: { name: string; category: string; mode: string }) => void;
  onOpenStudio?: (workflowId: string, recipeId: string) => void;
  onUseInProject?: (workflowId: string, recipeId: string) => void;
  onReturnToList?: () => void;
  onCancel?: () => void;
  onRetry?: () => void;
  importError?: WorkflowImportErrorView;
  projectId?: string;
  draft?: WorkflowOnboardingDraftView;
}

type PlanWithOptionalFormat = WorkflowAutoOnboardingPlanView & {
  format?: string;
  inputFormat?: string;
  sourceFormat?: string;
};

export function workflowImportFormat(plan: WorkflowAutoOnboardingPlanView): WorkflowImportFormat | undefined {
  const candidate = plan as PlanWithOptionalFormat;
  const value = String(candidate.format ?? candidate.inputFormat ?? candidate.sourceFormat ?? "").trim().toUpperCase();
  if (["API", "API_FORMAT"].includes(value)) return "API";
  if (["UI", "UI_FORMAT", "COMFY_UI"].includes(value)) return "UI";
  if (["NOT_API", "NOT_API_FORMAT", "WORKFLOW_NOT_API_FORMAT"].includes(value)) return "NOT_API";
  if (["INVALID", "INVALID_JSON", "MALFORMED_JSON"].includes(value)) return "INVALID_JSON";
  if (["UNKNOWN", "UNKNOWN_FORMAT", "UNRECOGNIZED"].includes(value)) return "UNKNOWN";

  const state = String(plan.state).trim().toUpperCase();
  if (["UNSUPPORTED_UI_FORMAT", "UI_FORMAT_UNSUPPORTED"].includes(state)) return "UI";
  if (["WORKFLOW_NOT_API_FORMAT", "NOT_API_FORMAT"].includes(state)) return "NOT_API";
  if (["INVALID_JSON", "INVALID_JSON_FORMAT"].includes(state)) return "INVALID_JSON";
  if (["UNKNOWN", "UNKNOWN_FORMAT"].includes(state)) return "UNKNOWN";
  return undefined;
}

function formatIssue(format: WorkflowImportFormat): WorkflowImportErrorView | undefined {
  if (format === "UI") {
    return {
      kind: "UI_FORMAT",
      message: "这个文件是 ComfyUI 普通工作流格式，不能安全地直接添加。",
    };
  }
  if (format === "NOT_API") {
    return {
      kind: "NOT_API_FORMAT",
      message: "该文件不是 ComfyUI API 格式工作流，请重新导出 API 格式工作流。",
    };
  }
  if (format === "INVALID_JSON") {
    return {
      kind: "INVALID_JSON",
      message: "它不是有效的 JSON 文件，请检查文件内容后重试。",
    };
  }
  if (format === "UNKNOWN") {
    return {
      kind: "UNKNOWN_FORMAT",
      message: "这个 JSON 不是可识别的 ComfyUI 工作流。",
    };
  }
  return undefined;
}

export function WorkflowSmartImport({ plan, draft, loading, onResolve, onResume, onOpenAdvanced, onOpenExisting, onOpenExistingVersion, onRegenerateRecipe, onRestoreExisting, onCommitImport, onSaveReviewMetadata, onOpenStudio, onUseInProject, onReturnToList, onCancel, onRetry, importError, projectId }: Props) {
  if (importError) {
    return <WorkflowImportFormatIssue issue={importError} loading={loading} onRetry={onRetry} onCancel={onReturnToList ?? onCancel} />;
  }
  if (!plan) return null;
  const detectedFormat = workflowImportFormat(plan);
  const pendingNormalization = plan.normalizationState === "UI_SOURCE_PENDING";
  const normalizedApiReady = plan.normalizationState === "NORMALIZED_API_READY";
  const detectedIssue = !pendingNormalization && !normalizedApiReady && detectedFormat && detectedFormat !== "API"
    ? formatIssue(detectedFormat)
    : undefined;
  if (detectedIssue) {
    return <WorkflowImportFormatIssue issue={detectedIssue} loading={loading} onRetry={onRetry} onCancel={onReturnToList ?? onCancel} />;
  }
  if (plan.state === "AUTO_PUBLISHED" || plan.published) {
    return (
      <WorkflowImportResult
        plan={plan}
        projectId={projectId}
        onOpenAdvanced={onOpenAdvanced}
        onOpenStudio={onOpenStudio}
        onUseInProject={onUseInProject}
        onReturnToList={onReturnToList}
      />
    );
  }
  return (
    <WorkflowImportIssues
      plan={plan}
      draft={draft}
      loading={loading}
      onResolve={onResolve}
      onResume={onResume}
      onOpenAdvanced={onOpenAdvanced}
      onOpenExisting={onOpenExisting}
      onOpenExistingVersion={onOpenExistingVersion}
      onUseInProject={projectId ? onUseInProject : undefined}
      onRegenerateRecipe={onRegenerateRecipe}
      onRestoreExisting={onRestoreExisting}
      onCommitImport={onCommitImport}
      onSaveReviewMetadata={onSaveReviewMetadata}
      onCancel={onCancel}
    />
  );
}
