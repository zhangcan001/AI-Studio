// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type {
  ComfyPreflightIssue,
  ComfyPreflightReport,
  ComfyPreflightWorkflowItem,
} from "../../types/settings";
import type { ProjectWorkflowBindingView, ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { KERA2_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { ProjectProductionReadiness } from "./ProjectProductionReadiness";

const mocks = vi.hoisted(() => ({
  getComfyPreflight: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getComfyPreflight: mocks.getComfyPreflight,
}));

function promptField(): RecipeField {
  return { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };
}

function imageRecipe(id: string): RecipeViewModel {
  return {
    workflowId: KERA2_WORKFLOW_ID,
    workflowVersionId: `image-${id}`,
    recipeId: `image-recipe-${id}`,
    name: `Image ${id}`,
    category: "image",
    mode: "text_to_image",
    fields: [promptField()],
    outputTypes: ["image"],
  };
}

function videoRecipe(id: string): RecipeViewModel {
  return {
    workflowId: `video-workflow-${id}`,
    workflowVersionId: `video-${id}`,
    recipeId: `video-recipe-${id}`,
    name: `Video ${id}`,
    category: "video",
    mode: "video",
    fields: [
      promptField(),
      { key: "first_frame", type: "image", label: "First", required: false },
      { key: "last_frame", type: "image", label: "Last", required: false },
      { key: "reference_image", type: "image", label: "Reference image", required: false },
      { key: "reference_video", type: "video", label: "Reference video", required: false },
      { key: "reference_audio", type: "audio", label: "Reference audio", required: false },
    ],
    outputTypes: ["video"],
  };
}

function binding(
  recipe: RecipeViewModel,
  stage: "IMAGE" | "VIDEO",
  mode: "DEFAULT" | "FL2VA_TEXT_TO_VIDEO" = "DEFAULT",
): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    available: true,
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: "project-readiness", videoModeOverrides: [], ...patch };
}

function runtimeItem(
  recipe: RecipeViewModel,
  status = "READY",
  patch: Partial<ComfyPreflightWorkflowItem> = {},
): ComfyPreflightWorkflowItem {
  return {
    workflowId: recipe.workflowId,
    workflowVersionId: recipe.workflowVersionId,
    name: recipe.name,
    version: "1",
    status,
    missingNodes: [],
    reason: null,
    ...patch,
  };
}

function issue(patch: Partial<ComfyPreflightIssue> = {}): ComfyPreflightIssue {
  return {
    severity: "WARNING",
    code: "RUNTIME_WARNING",
    title: "运行时提醒",
    detail: "当前工作流需要注意。",
    workflowId: null,
    workflowVersionId: null,
    missingNodes: null,
    suggestedAction: "完成真实验证。",
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

const IMAGE_A = imageRecipe("a");
const IMAGE_B = imageRecipe("b");
const VIDEO_A = videoRecipe("a");
const CATALOG = [IMAGE_A, IMAGE_B, VIDEO_A];
const READY_RUNTIME = runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)]);
const IMAGE_SHARED_A = { ...IMAGE_A, workflowVersionId: "image-shared", recipeId: "image-recipe-a" };
const IMAGE_SHARED_B = { ...IMAGE_B, workflowVersionId: "image-shared", recipeId: "image-recipe-b" };
const SHARED_RECIPE_CATALOG = [IMAGE_SHARED_A, IMAGE_SHARED_B, VIDEO_A];
const READY_SHARED_RUNTIME = runtimeReport([runtimeItem(IMAGE_SHARED_A), runtimeItem(VIDEO_A)]);

async function check() {
  await userEvent.click(screen.getByRole("button", { name: /检查开工条件/ }));
}

describe("ProjectProductionReadiness", () => {
  beforeEach(() => {
    mocks.getComfyPreflight.mockReset();
  });

  afterEach(cleanup);

  it("shows an unchecked state and does not run preflight on mount", () => {
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(screen.getByRole("button", { name: "检查开工条件" })).toBeTruthy();
    expect(mocks.getComfyPreflight).not.toHaveBeenCalled();
  });

  it("shows READY summary after one explicit check", async () => {
    mocks.getComfyPreflight.mockResolvedValue(READY_RUNTIME);
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();

    await waitFor(() => expect(screen.getByText("✓ 项目可以开工")).toBeTruthy());
    expect(screen.getByText("8 / 8 条生产路径具备开工条件")).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("shows PARTIAL summary when only one project path has a recipe", async () => {
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([runtimeItem(IMAGE_A)]));
    render(<ProjectProductionReadiness config={config()} catalog={[IMAGE_A]} />);

    await check();

    await waitFor(() => expect(screen.getByText("⚠ 项目部分路径可以开工")).toBeTruthy());
    expect(screen.getByText("1 / 8 条生产路径具备开工条件")).toBeTruthy();
  });

  it("shows BUSY without blocking paths or starting production", async () => {
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)], {
      runtimeBusy: true,
      activeTaskCount: 1,
      productionBusy: true,
    }));
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();

    await waitFor(() => expect(screen.getByText("⏳ 运行环境忙碌")).toBeTruthy());
    expect(screen.getByText("活动任务").nextElementSibling?.textContent).toBe("1");
    expect(screen.getByText(/待运行环境空闲后/)).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("shows BLOCKED and missing nodes when the only runtime workflow is blocked", async () => {
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([
      runtimeItem(VIDEO_A, "BLOCKED", { missingNodes: ["NodeA"], reason: "缺少节点 NodeA" }),
    ], { status: "BLOCKED" }));
    render(<ProjectProductionReadiness config={config()} catalog={[VIDEO_A]} />);

    await check();

    await waitFor(() => expect(screen.getByText("✕ 当前项目无法开工")).toBeTruthy());
    expect(screen.getAllByText(/NodeA/).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Runtime：BLOCKED").length).toBeGreaterThan(0);
  });

  it("shows DEGRADED as a runnable warning and filters unrelated issues", async () => {
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([
      runtimeItem(IMAGE_A),
      runtimeItem(VIDEO_A, "DEGRADED", { reason: "尚未完成真实生成验证" }),
    ], {
      issues: [
        issue({ workflowVersionId: VIDEO_A.workflowVersionId }),
        issue({ code: "UNRELATED", workflowId: "unrelated-workflow", workflowVersionId: "unrelated-version" }),
      ],
    }));
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();

    await waitFor(() => expect(screen.getAllByText(/可开工，但需要注意/).length).toBeGreaterThan(0));
    expect(screen.getAllByText(/尚未完成真实生成验证/).length).toBeGreaterThan(0);
    expect(screen.getByText("当前项目相关运行问题")).toBeTruthy();
    expect(screen.queryByText("UNRELATED")).toBeNull();
  });

  it("updates the report on explicit recheck", async () => {
    mocks.getComfyPreflight
      .mockResolvedValueOnce(READY_RUNTIME)
      .mockResolvedValueOnce(runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)], {
        connection: "OFFLINE",
        status: "BLOCKED",
      }));
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();
    await waitFor(() => expect(screen.getByText("ComfyUI").parentElement?.textContent).toContain("已连接"));
    await userEvent.click(screen.getByRole("button", { name: "重新检查开工条件" }));

    await waitFor(() => expect(screen.getByText("ComfyUI").parentElement?.textContent).toContain("离线"));
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(2);
  });

  it("shows first-check errors without fabricating BLOCKED", async () => {
    mocks.getComfyPreflight.mockRejectedValue(new Error("offline"));
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("开工检查失败"));
    expect(screen.queryByText("✕ 当前项目无法开工")).toBeNull();
  });

  it("keeps a successful report when recheck fails", async () => {
    mocks.getComfyPreflight
      .mockResolvedValueOnce(READY_RUNTIME)
      .mockRejectedValueOnce(new Error("offline"));
    render(<ProjectProductionReadiness config={config()} catalog={CATALOG} />);

    await check();
    await waitFor(() => expect(screen.getByText("8 / 8 条生产路径具备开工条件")).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: "重新检查开工条件" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("重新检查失败"));
    expect(screen.getByText("8 / 8 条生产路径具备开工条件")).toBeTruthy();
  });

  it("invalidates the old runtime snapshot when the project workflow changes", async () => {
    mocks.getComfyPreflight.mockResolvedValue(READY_RUNTIME);
    const { rerender } = render(
      <ProjectProductionReadiness config={config({ imageDefault: binding(IMAGE_A, "IMAGE") })} catalog={CATALOG} />,
    );

    await check();
    await waitFor(() => expect(screen.getByText(/WorkflowVersion：image-a/)).toBeTruthy());
    rerender(<ProjectProductionReadiness config={config({ imageDefault: binding(IMAGE_B, "IMAGE") })} catalog={CATALOG} />);

    await waitFor(() => expect(screen.getByText("项目工作流已变化，请重新检查开工条件。")).toBeTruthy());
    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("invalidates the old runtime snapshot when only the configured recipe changes", async () => {
    mocks.getComfyPreflight.mockResolvedValue(READY_SHARED_RUNTIME);
    const { rerender } = render(
      <ProjectProductionReadiness
        config={config({ imageDefault: binding(IMAGE_SHARED_A, "IMAGE") })}
        catalog={SHARED_RECIPE_CATALOG}
      />,
    );

    await check();
    await waitFor(() => expect(screen.getByText(/WorkflowVersion：image-shared/)).toBeTruthy());
    rerender(<ProjectProductionReadiness config={config({ imageDefault: binding(IMAGE_SHARED_B, "IMAGE") })} catalog={SHARED_RECIPE_CATALOG} />);

    await waitFor(() => expect(screen.getByText("项目工作流已变化，请重新检查开工条件。")).toBeTruthy());
    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("invalidates the old runtime snapshot when the same recipe changes preflight semantics", async () => {
    mocks.getComfyPreflight.mockResolvedValue(READY_RUNTIME);
    const freshConfig = config({ imageDefault: binding(IMAGE_A, "IMAGE") });
    const staleConfig = config({ imageDefault: { ...binding(IMAGE_A, "IMAGE"), available: false } });
    const { rerender } = render(<ProjectProductionReadiness config={freshConfig} catalog={CATALOG} />);

    await check();
    await waitFor(() => expect(screen.getAllByTestId("project-production-readiness-path")[0].className).toContain("-ready"));
    rerender(<ProjectProductionReadiness config={staleConfig} catalog={CATALOG} />);

    await waitFor(() => expect(screen.getByText("项目工作流已变化，请重新检查开工条件。")).toBeTruthy());
    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });
});
