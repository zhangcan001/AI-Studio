// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../../types/generation";
import type { ProjectWorkflowConfigView } from "../../../types/projectWorkflow";
import {
  MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_WORKFLOW_ID,
} from "../../runtime/productRuntimeScope";
import { recipeRef, type H3CompatibleMode, type SelectedRecipeRef } from "../../runtime/workflowCapabilities";
import { useAssetVideoWorkflowController } from "./useAssetVideoWorkflowController";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return { ...actual, getProjectWorkflowConfig: mocks.getProjectWorkflowConfig };
});

const T2V_MODE: H3CompatibleMode = "FL2VA_TEXT_TO_VIDEO";
const I2V_MODE: H3CompatibleMode = "FL2VA_IMAGE_TO_VIDEO";

function h3Recipe(
  workflowId: string,
  workflowVersionId: string,
  recipeId: string,
  withFirstFrame = false,
): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId,
    recipeId,
    name: "同名工作流",
    category: "video",
    mode: "h3",
    fields: [
      { key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 15, step: 1 },
      { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 },
      { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 },
      { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
      ...(withFirstFrame ? [{ key: "first_frame", type: "image" as const, label: "首帧", required: false }] : []),
      { key: "seed", type: "seed", label: "种子", defaultMode: "random" },
    ],
    outputTypes: ["video"],
  };
}

const t2v = h3Recipe(MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID, "version-t2v", "recipe-t2v");
const t2vAlternative = h3Recipe(MINIMAX_H3_FL2VA_WORKFLOW_ID, "version-alt", "recipe-alt");
const i2v = h3Recipe(MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID, "version-i2v", "recipe-i2v", true);
const catalog = [t2v, t2vAlternative, i2v];

function config(projectId: string, overrides: ProjectWorkflowConfigView["videoModeOverrides"] = [], videoDefault?: ProjectWorkflowConfigView["videoDefault"]): ProjectWorkflowConfigView {
  return { projectId, videoModeOverrides: overrides, videoDefault };
}

function binding(ref: SelectedRecipeRef, mode: ProjectWorkflowConfigView["videoModeOverrides"][number]["mode"] = T2V_MODE, available = true) {
  return {
    stage: "VIDEO" as const,
    mode,
    workflowVersionId: ref.workflowVersionId,
    recipeId: ref.recipeId,
    available,
    createdAt: "2026-09-09T00:00:00Z",
    updatedAt: "2026-09-09T00:00:00Z",
  };
}

function Harness({
  projectId = "project-a",
  generationMode = T2V_MODE,
  projectModes = [T2V_MODE],
}: {
  projectId?: string;
  generationMode?: H3CompatibleMode;
  projectModes?: H3CompatibleMode[];
}) {
  const controller = useAssetVideoWorkflowController({
    projectId,
    catalog,
    generationMode,
    qualityProfile: "QUALITY",
    projectModes,
  });
  const resolved = controller.resolvedProjectRecipes[0];
  return (
    <>
      <output data-testid="recipe">{controller.recipe?.recipeId ?? "none"}</output>
      <output data-testid="source">{controller.workflowSelectionSource ?? "none"}</output>
      <output data-testid="notice">{controller.workflowSelectionNotice ?? ""}</output>
      <output data-testid="stale-project">{String(controller.staleProjectVideoBinding)}</output>
      <output data-testid="strategy">{controller.projectWorkflowStrategy}</output>
      <output data-testid="config-project">{controller.projectWorkflowConfig?.projectId ?? "loading"}</output>
      <output data-testid="overrides">{JSON.stringify(controller.projectManualOverrides)}</output>
      <output data-testid="project-recipe">{resolved?.recipe?.recipeId ?? "none"}</output>
      <button type="button" onClick={() => controller.selectVideoWorkflow(t2vAlternative)}>select-manual</button>
      <button type="button" onClick={controller.restoreRecommendedVideoWorkflow}>restore-recommended</button>
      <button type="button" onClick={() => controller.setProjectWorkflowStrategy("MANUAL")}>set-manual-strategy</button>
      <button type="button" onClick={() => controller.setProjectWorkflowStrategy("AUTO")}>set-auto-strategy</button>
      <button type="button" onClick={() => controller.setProjectWorkflowOverride(T2V_MODE, recipeRef(t2vAlternative))}>set-override</button>
      <button type="button" onClick={() => controller.setProjectWorkflowOverride(T2V_MODE, undefined)}>clear-override</button>
    </>
  );
}

describe("useAssetVideoWorkflowController", () => {
  afterEach(cleanup);

  function expectText(testId: string, text: string) {
    expect(screen.getByTestId(testId).textContent).toContain(text);
  }

  beforeEach(() => {
    mocks.getProjectWorkflowConfig.mockReset();
    mocks.getProjectWorkflowConfig.mockResolvedValue(config("project-a"));
  });

  it("uses the recommended exact recipe when no binding or manual choice exists", async () => {
    render(<Harness />);

    await waitFor(() => expectText("recipe", "recipe-t2v"));
    expectText("source", "recommended");
  });

  it("preserves an explicit recipe and clears it when the generation mode becomes stale", async () => {
    const view = render(<Harness />);
    await waitFor(() => expectText("config-project", "project-a"));

    fireEvent.click(screen.getByRole("button", { name: "select-manual" }));
    expectText("recipe", "recipe-alt");
    expectText("source", "manual");

    view.rerender(<Harness generationMode={I2V_MODE} projectModes={[I2V_MODE]} />);
    await waitFor(() => expectText("recipe", "recipe-i2v"));
    expectText("notice", "当前工作流不支持此生成模式，已切换到兼容工作流。");
  });

  it("keeps project mode and project default resolution ahead of the recommendation", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", [binding(recipeRef(t2vAlternative))]));
    const firstView = render(<Harness />);
    await waitFor(() => expectText("recipe", "recipe-alt"));
    expectText("source", "project_mode");
    firstView.unmount();

    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-b", [], binding(recipeRef(t2vAlternative))));
    const view = render(<Harness projectId="project-b" />);
    await waitFor(() => expectText("recipe", "recipe-alt"));
    expectText("source", "project_default");
    view.unmount();
  });

  it("reports stale project bindings without repairing them", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", [], binding(recipeRef(t2v), T2V_MODE, false)));
    render(<Harness />);
    await waitFor(() => expectText("stale-project", "true"));
    expectText("notice", "项目工作流绑定已失效");
  });

  it("ignores manual project overrides in AUTO and honors exact overrides in MANUAL", async () => {
    render(<Harness />);
    await waitFor(() => expectText("config-project", "project-a"));

    fireEvent.click(screen.getByRole("button", { name: "set-override" }));
    expectText("project-recipe", "recipe-t2v");
    fireEvent.click(screen.getByRole("button", { name: "set-manual-strategy" }));
    expectText("project-recipe", "recipe-alt");
    expectText("overrides", "recipe-alt");
    fireEvent.click(screen.getByRole("button", { name: "clear-override" }));
    expectText("overrides", "{}");
  });

  it("resets local workflow state and loads the new project config", async () => {
    const view = render(<Harness />);
    await waitFor(() => expectText("config-project", "project-a"));
    fireEvent.click(screen.getByRole("button", { name: "set-manual-strategy" }));
    fireEvent.click(screen.getByRole("button", { name: "set-override" }));
    expectText("strategy", "MANUAL");

    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-b"));
    view.rerender(<Harness projectId="project-b" />);
    await waitFor(() => expectText("config-project", "project-b"));
    expectText("strategy", "AUTO");
    expectText("overrides", "{}");
  });

  it("matches bindings by workflowVersionId and recipeId, not by name", async () => {
    const sameNameDifferentVersion = binding({ workflowVersionId: "wrong-version", recipeId: "recipe-t2v" });
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", [sameNameDifferentVersion]));
    render(<Harness />);
    await waitFor(() => expectText("recipe", "recipe-t2v"));
    expectText("source", "recommended");
    expectText("stale-project", "true");
  });

  it("clears an override key instead of retaining an undefined value", async () => {
    render(<Harness />);
    await waitFor(() => expectText("config-project", "project-a"));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "set-override" }));
      fireEvent.click(screen.getByRole("button", { name: "clear-override" }));
    });
    expectText("overrides", "{}");
  });
});
