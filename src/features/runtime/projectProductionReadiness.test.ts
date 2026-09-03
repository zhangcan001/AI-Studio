import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
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
import {
  composeProjectProductionReadiness,
} from "./projectProductionReadiness";

const PATHS: ProjectWorkflowProductionPath[] = [
  "IMAGE",
  "FL2VA_TEXT_TO_VIDEO",
  "FL2VA_IMAGE_TO_VIDEO",
  "FL2VA_FIRST_LAST",
  "REF2VA_IMAGE",
  "REF2VA_AUDIO",
  "REF2VA_IMAGE_AUDIO",
  "REF2VA_VIDEO_IMAGE",
];

function recipe(id: string): RecipeViewModel {
  return {
    workflowId: `workflow-${id}`,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: `Recipe ${id}`,
    category: "video",
    mode: "video",
    fields: [],
    outputTypes: ["video"],
  };
}

function projectItem(
  path: ProjectWorkflowProductionPath,
  projectRecipe: RecipeViewModel | undefined,
  status: ProjectWorkflowPreflightItem["status"] = projectRecipe ? "READY" : "BLOCKED",
): ProjectWorkflowPreflightItem {
  return {
    path,
    status,
    recipe: projectRecipe,
    staleConfiguredBinding: status === "WARNING",
    usingFallback: status === "WARNING",
    message: status === "BLOCKED" ? "当前无兼容工作流。" : status === "WARNING" ? "项目绑定不可用，当前使用系统推荐。" : "当前路径可生产。",
  };
}

function projectReport(
  items: ProjectWorkflowPreflightItem[] = PATHS.map((path, index) => projectItem(path, recipe(String(index)))),
): ProjectWorkflowPreflightReport {
  const blockedCount = items.filter((item) => item.status === "BLOCKED").length;
  const readyCount = items.filter((item) => item.recipe !== undefined).length;
  return {
    overallStatus: blockedCount === 0 ? "READY" : readyCount ? "PARTIAL" : "BLOCKED",
    readyCount,
    warningCount: items.filter((item) => item.status === "WARNING").length,
    blockedCount,
    totalCount: 8,
    items,
  };
}

function runtimeItem(
  projectRecipe: RecipeViewModel,
  status = "READY",
  patch: Partial<ComfyPreflightWorkflowItem> = {},
): ComfyPreflightWorkflowItem {
  return {
    workflowId: projectRecipe.workflowId,
    workflowVersionId: projectRecipe.workflowVersionId,
    name: projectRecipe.name,
    version: "1",
    status,
    missingNodes: [],
    reason: null,
    ...patch,
  };
}

function issue(patch: Partial<ComfyPreflightIssue> = {}): ComfyPreflightIssue {
  return {
    severity: "ERROR",
    code: "WORKFLOW_BLOCKED",
    title: "工作流不可用",
    detail: "当前工作流存在问题。",
    workflowId: null,
    workflowVersionId: null,
    missingNodes: null,
    suggestedAction: null,
    ...patch,
  };
}

function runtimeReport(
  items: ComfyPreflightWorkflowItem[],
  patch: Partial<ComfyPreflightReport> = {},
): ComfyPreflightReport {
  return {
    endpoint: "http://127.0.0.1:8188",
    status: "READY",
    checkedAt: "2026-09-03T10:00:00Z",
    connection: "CONNECTED",
    comfyuiVersion: "0.33.0",
    pythonVersion: "3.12.10",
    gpu: "NVIDIA Test GPU",
    vramTotal: 16 * 1024 ** 3,
    vramFree: 8 * 1024 ** 3,
    nodeCount: 4516,
    runtimeBusy: false,
    activeTaskCount: 0,
    productionBusy: false,
    workflowSummary: {
      workflowTotal: items.length,
      workflowReady: items.filter((item) => item.status === "READY").length,
      workflowBlocked: items.filter((item) => item.status === "BLOCKED").length,
      items,
    },
    issues: [],
    ...patch,
  };
}

function runtimeFor(project: ProjectWorkflowPreflightReport): ComfyPreflightWorkflowItem[] {
  return project.items.flatMap((item) => item.recipe ? [runtimeItem(item.recipe)] : []);
}

describe("project production readiness", () => {
  it("A1: reports READY when all eight project paths match ready runtime workflows", () => {
    const project = projectReport();
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project)));

    expect(result).toMatchObject({ status: "READY", totalCount: 8, runnablePathCount: 8, warningPathCount: 0, blockedPathCount: 0 });
    expect(result.paths.every((path) => path.status === "READY")).toBe(true);
  });

  it("A2: reports PARTIAL when five paths are runnable and three are project-blocked", () => {
    const project = projectReport([
      ...PATHS.slice(0, 5).map((path, index) => projectItem(path, recipe(String(index)))),
      ...PATHS.slice(5).map((path) => projectItem(path, undefined)),
    ]);
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project)));

    expect(result).toMatchObject({ status: "PARTIAL", runnablePathCount: 5, blockedPathCount: 3 });
  });

  it("A3: reports BUSY without changing runnable path results", () => {
    const project = projectReport([
      ...PATHS.slice(0, 6).map((path, index) => projectItem(path, recipe(String(index)))),
      ...PATHS.slice(6).map((path) => projectItem(path, undefined)),
    ]);
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project), {
      runtimeBusy: true,
      activeTaskCount: 1,
      productionBusy: true,
    }));

    expect(result.status).toBe("BUSY");
    expect(result.runnablePathCount).toBe(6);
    expect(result.paths.slice(0, 6).every((path) => path.status === "READY")).toBe(true);
  });

  it("A4: blocks all configured paths when ComfyUI is offline", () => {
    const project = projectReport();
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project), {
      connection: "OFFLINE",
      status: "BLOCKED",
    }));

    expect(result.status).toBe("BLOCKED");
    expect(result.paths.every((path) => path.status === "BLOCKED")).toBe(true);
    expect(result.paths[0].reasons[0]).toContain("OFFLINE");
  });

  it("A5: blocks configured paths when node capability is unknown", () => {
    const project = projectReport();
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project), { nodeCount: null }));

    expect(result.status).toBe("BLOCKED");
    expect(result.paths.map((path) => path.reasons[0]).every((reason) => reason.includes("节点能力"))).toBe(true);
  });

  it("A6: preserves matching runtime BLOCKED missing nodes", () => {
    const project = projectReport();
    const blockedRecipe = project.items[0].recipe!;
    const result = composeProjectProductionReadiness(project, runtimeReport([
      runtimeItem(blockedRecipe, "BLOCKED", { missingNodes: ["NodeA"], reason: "缺少节点 NodeA" }),
      ...runtimeFor(project).slice(1),
    ], { issues: [issue({ workflowVersionId: blockedRecipe.workflowVersionId, missingNodes: ["NodeA"] })] }));

    expect(result.paths[0]).toMatchObject({ status: "BLOCKED", runtimeWorkflow: { workflowVersionId: blockedRecipe.workflowVersionId } });
    expect(result.paths[0].reasons.join(" ")).toContain("NodeA");
    expect(result.paths[0].issues[0].workflowVersionId).toBe(blockedRecipe.workflowVersionId);
  });

  it("A7: maps a disabled runtime workflow to BLOCKED", () => {
    const project = projectReport();
    const disabledRecipe = project.items[0].recipe!;
    const result = composeProjectProductionReadiness(project, runtimeReport([
      runtimeItem(disabledRecipe, "DISABLED"),
      ...runtimeFor(project).slice(1),
    ]));

    expect(result.paths[0].status).toBe("BLOCKED");
    expect(result.paths[0].reasons[0]).toContain("停用");
  });

  it("A8: maps DEGRADED to a runnable WARNING", () => {
    const project = projectReport();
    const degradedRecipe = project.items[0].recipe!;
    const result = composeProjectProductionReadiness(project, runtimeReport([
      runtimeItem(degradedRecipe, "DEGRADED", { reason: "尚未完成真实生成验证" }),
      ...runtimeFor(project).slice(1),
    ]));

    expect(result).toMatchObject({ status: "READY", runnablePathCount: 8, warningPathCount: 1 });
    expect(result.paths[0]).toMatchObject({ status: "WARNING", reasons: ["尚未完成真实生成验证"] });
  });

  it("A9: filters unrelated workflow issues and ignores the global runtime status", () => {
    const project = projectReport();
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project), {
      status: "BLOCKED",
      issues: [
        issue({ workflowId: "workflow-z", workflowVersionId: "version-z" }),
        issue({ code: "COMFY_WARNING", workflowId: null, workflowVersionId: null, severity: "WARNING" }),
      ],
    }));

    expect(result.status).toBe("READY");
    expect(result.relevantIssues).toHaveLength(1);
    expect(result.relevantIssues[0].code).toBe("COMFY_WARNING");
  });

  it("A10: blocks when the exact project WorkflowVersion is absent from the runtime workspace", () => {
    const project = projectReport();
    const missingRecipe = project.items[0].recipe!;
    const runtimeReplacement = runtimeItem({ ...missingRecipe, workflowVersionId: "version-other", name: missingRecipe.name });
    const result = composeProjectProductionReadiness(project, runtimeReport([
      runtimeReplacement,
      ...runtimeFor(project).slice(1),
    ]));

    expect(result.paths[0].status).toBe("BLOCKED");
    expect(result.paths[0].reasons[0]).toContain("未找到该 WorkflowVersion");
  });

  it("A11: keeps a stale project fallback as runnable WARNING when runtime is ready", () => {
    const project = projectReport([
      projectItem(PATHS[0], recipe("fallback"), "WARNING"),
      ...PATHS.slice(1).map((path, index) => projectItem(path, recipe(String(index + 1)))),
    ]);
    const result = composeProjectProductionReadiness(project, runtimeReport(runtimeFor(project)));

    expect(result).toMatchObject({ status: "READY", runnablePathCount: 8, warningPathCount: 1, blockedPathCount: 0 });
    expect(result.paths[0].status).toBe("WARNING");
  });
});
