import type {
  ComfyPreflightIssue,
  ComfyPreflightReport,
  ComfyPreflightWorkflowItem,
} from "../../types/settings";
import type {
  ProjectWorkflowPreflightItem,
  ProjectWorkflowPreflightReport,
  ProjectWorkflowProductionPath,
} from "./projectWorkflowPreflight";

export type ProjectProductionReadinessStatus = "READY" | "PARTIAL" | "BUSY" | "BLOCKED";

export type ProjectProductionPathReadinessStatus = "READY" | "WARNING" | "BLOCKED";

export interface ProjectProductionPathReadiness {
  path: ProjectWorkflowProductionPath;
  status: ProjectProductionPathReadinessStatus;
  projectWorkflow: ProjectWorkflowPreflightItem;
  runtimeWorkflow?: ComfyPreflightWorkflowItem;
  reasons: string[];
  issues: ComfyPreflightIssue[];
}

export interface ProjectProductionReadinessReport {
  status: ProjectProductionReadinessStatus;
  totalCount: 8;
  runnablePathCount: number;
  warningPathCount: number;
  blockedPathCount: number;
  runtimeBusy: boolean;
  activeTaskCount: number;
  productionBusy: boolean;
  connection: ComfyPreflightReport["connection"];
  endpoint: string;
  gpu?: string | null;
  vramTotal?: number | null;
  vramFree?: number | null;
  nodeCount?: number | null;
  runtimeCheckedAt: string;
  paths: ProjectProductionPathReadiness[];
  relevantIssues: ComfyPreflightIssue[];
}

function relevantIssues(
  issues: ComfyPreflightIssue[],
  workflowVersionIds: Set<string>,
  workflowIds: Set<string>,
): ComfyPreflightIssue[] {
  return issues.filter((issue) => {
    if (issue.workflowVersionId != null) return workflowVersionIds.has(issue.workflowVersionId);
    if (issue.workflowId != null) return workflowIds.has(issue.workflowId);
    return true;
  });
}

function pathIssues(
  issues: ComfyPreflightIssue[],
  recipe: ProjectWorkflowPreflightItem["recipe"],
): ComfyPreflightIssue[] {
  if (!recipe) return [];
  return issues.filter((issue) => {
    if (issue.workflowVersionId != null) return issue.workflowVersionId === recipe.workflowVersionId;
    if (issue.workflowId != null) return issue.workflowId === recipe.workflowId;
    return false;
  });
}

function connectionReason(connection: ComfyPreflightReport["connection"]): string {
  return connection === "OFFLINE"
    ? "ComfyUI OFFLINE：当前无法连接。"
    : "ComfyUI INCOMPATIBLE：当前接口不兼容。";
}

function runtimeReasons(runtimeWorkflow: ComfyPreflightWorkflowItem): string[] {
  const reasons = runtimeWorkflow.reason ? [runtimeWorkflow.reason] : [];
  if (runtimeWorkflow.missingNodes?.length) reasons.push(`缺少节点：${runtimeWorkflow.missingNodes.join("、")}`);
  return reasons;
}

function pathReadiness(
  projectWorkflow: ProjectWorkflowPreflightItem,
  runtimeReport: ComfyPreflightReport,
  relevantRuntimeIssues: ComfyPreflightIssue[],
): ProjectProductionPathReadiness {
  const recipe = projectWorkflow.recipe;
  if (!recipe || projectWorkflow.status === "BLOCKED") {
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      reasons: [projectWorkflow.message],
      issues: [],
    };
  }

  const issues = pathIssues(relevantRuntimeIssues, recipe);
  if (runtimeReport.connection !== "CONNECTED") {
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      reasons: [connectionReason(runtimeReport.connection)],
      issues,
    };
  }
  if (runtimeReport.nodeCount == null) {
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      reasons: ["当前无法确认 ComfyUI 节点能力。"],
      issues,
    };
  }

  const runtimeWorkflow = runtimeReport.workflowSummary.items?.find((item) => (
    item.workflowVersionId === recipe.workflowVersionId
  ));
  if (!runtimeWorkflow) {
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      reasons: ["当前 Runtime Workspace 中未找到该 WorkflowVersion。请检查工作流运行包或重新刷新工作流库。"],
      issues,
    };
  }

  const projectReasons = projectWorkflow.status === "WARNING" ? [projectWorkflow.message] : [];
  if (runtimeWorkflow.status === "BLOCKED") {
    const reasons = runtimeReasons(runtimeWorkflow);
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      runtimeWorkflow,
      reasons: reasons.length ? reasons : ["该 WorkflowVersion 当前不可用。"],
      issues,
    };
  }
  if (runtimeWorkflow.status === "DISABLED") {
    return {
      path: projectWorkflow.path,
      status: "BLOCKED",
      projectWorkflow,
      runtimeWorkflow,
      reasons: ["该 WorkflowVersion 当前已停用。", ...runtimeReasons(runtimeWorkflow)],
      issues,
    };
  }
  if (runtimeWorkflow.status !== "READY") {
    const reasons = [...projectReasons, ...runtimeReasons(runtimeWorkflow)];
    return {
      path: projectWorkflow.path,
      status: "WARNING",
      projectWorkflow,
      runtimeWorkflow,
      reasons: reasons.length ? reasons : [`运行时状态：${runtimeWorkflow.status}。`],
      issues,
    };
  }
  if (projectReasons.length) {
    return {
      path: projectWorkflow.path,
      status: "WARNING",
      projectWorkflow,
      runtimeWorkflow,
      reasons: projectReasons,
      issues,
    };
  }
  return {
    path: projectWorkflow.path,
    status: "READY",
    projectWorkflow,
    runtimeWorkflow,
    reasons: [],
    issues,
  };
}

export function composeProjectProductionReadiness(
  projectReport: ProjectWorkflowPreflightReport,
  runtimeReport: ComfyPreflightReport,
): ProjectProductionReadinessReport {
  const projectRecipes = projectReport.items.flatMap((item) => item.recipe ? [item.recipe] : []);
  const projectWorkflowVersionIds = new Set(projectRecipes.map((recipe) => recipe.workflowVersionId));
  const projectWorkflowIds = new Set(projectRecipes.map((recipe) => recipe.workflowId));
  const filteredIssues = relevantIssues(
    runtimeReport.issues,
    projectWorkflowVersionIds,
    projectWorkflowIds,
  );
  const paths = projectReport.items.map((item) => pathReadiness(item, runtimeReport, filteredIssues));
  const warningPathCount = paths.filter((path) => path.status === "WARNING").length;
  const blockedPathCount = paths.filter((path) => path.status === "BLOCKED").length;
  const runnablePathCount = paths.length - blockedPathCount;
  const status: ProjectProductionReadinessStatus = runnablePathCount === 0
    ? "BLOCKED"
    : runtimeReport.runtimeBusy
      ? "BUSY"
      : blockedPathCount > 0
        ? "PARTIAL"
        : "READY";
  return {
    status,
    totalCount: 8,
    runnablePathCount,
    warningPathCount,
    blockedPathCount,
    runtimeBusy: runtimeReport.runtimeBusy,
    activeTaskCount: runtimeReport.activeTaskCount,
    productionBusy: runtimeReport.productionBusy,
    connection: runtimeReport.connection,
    endpoint: runtimeReport.endpoint,
    gpu: runtimeReport.gpu,
    vramTotal: runtimeReport.vramTotal,
    vramFree: runtimeReport.vramFree,
    nodeCount: runtimeReport.nodeCount,
    runtimeCheckedAt: runtimeReport.checkedAt,
    paths,
    relevantIssues: filteredIssues,
  };
}
