// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../../types/generation";
import type { ProjectWorkflowConfigView } from "../../../types/projectWorkflow";
import { useStudioStore } from "../../../stores/studioStore";
import { KERA2_WORKFLOW_ID } from "../../runtime/productRuntimeScope";
import { recipeRef } from "../../runtime/workflowCapabilities";
import { useGenerationWorkflowSelectionController } from "./useGenerationWorkflowSelectionController";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return { ...actual, getProjectWorkflowConfig: mocks.getProjectWorkflowConfig };
});

function imageRecipe(
  suffix: string,
  patch: Partial<RecipeViewModel> = {},
): RecipeViewModel {
  return {
    workflowId: `workflow-${suffix}`,
    workflowVersionId: `version-${suffix}`,
    recipeId: `recipe-${suffix}`,
    name: `Workflow ${suffix}`,
    category: "image",
    mode: "text-to-image",
    fields: [
      { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
      { key: "width", type: "integer", label: "宽度", required: true, default: 512, min: 64, max: 2048, step: 64 },
    ],
    outputTypes: ["image"],
    ...patch,
  };
}

function kera2Recipe(): RecipeViewModel {
  return imageRecipe("kera2", {
    workflowId: KERA2_WORKFLOW_ID,
    fields: [
      { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
      { key: "width", type: "integer", label: "宽度", required: true, default: 1024, min: 64, max: 2048, step: 64 },
      { key: "height", type: "integer", label: "高度", required: true, default: 1024, min: 64, max: 2048, step: 64 },
      { key: "seed", type: "seed", label: "种子", defaultMode: "random" },
    ],
  });
}

const first = imageRecipe("first");
const second = imageRecipe("second");
const third = imageRecipe("third");

function config(projectId: string, imageDefault?: ProjectWorkflowConfigView["imageDefault"]): ProjectWorkflowConfigView {
  return { projectId, imageDefault, videoModeOverrides: [] };
}

type HookOptions = Parameters<typeof useGenerationWorkflowSelectionController>[0];

function options(overrides: Partial<HookOptions> = {}): HookOptions {
  return {
    projectId: "project-a",
    productCatalog: [first, second],
    selectedWorkflow: undefined,
    draftDirty: false,
    onNotice: vi.fn(),
    onWorkflowChanged: vi.fn(),
    ...overrides,
  };
}

function imageBinding(recipe: RecipeViewModel, available = true): NonNullable<ProjectWorkflowConfigView["imageDefault"]> {
  return {
    stage: "IMAGE",
    mode: "DEFAULT",
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    available,
    createdAt: "2026-09-10T00:00:00Z",
    updatedAt: "2026-09-10T00:00:00Z",
  };
}

describe("useGenerationWorkflowSelectionController", () => {
  beforeEach(() => {
    useStudioStore.getState().resetDraft();
    mocks.getProjectWorkflowConfig.mockReset();
    mocks.getProjectWorkflowConfig.mockImplementation((projectId: string) => Promise.resolve(config(projectId)));
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("loads the exact project config and resets local selection state on project changes", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", imageBinding(first)));
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options(),
    });
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-a"));
    expect(view.result.current.projectDefaultWorkflow?.recipeId).toBe(first.recipeId);

    view.rerender(options({ projectId: "project-b" }));
    expect(view.result.current.projectWorkflowConfig).toBeUndefined();
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-b"));
    expect(view.result.current.projectDefaultWorkflow).toBeUndefined();
  });

  it("uses the empty fallback config when loading fails", async () => {
    mocks.getProjectWorkflowConfig.mockRejectedValueOnce(new Error("offline"));
    const view = renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-a"));
    expect(view.result.current.projectWorkflowConfig?.videoModeOverrides).toEqual([]);
  });

  it("falls back to the recommended workflow when the project has no image default", async () => {
    const view = renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-a"));
    expect(view.result.current.projectDefaultWorkflow).toBeUndefined();
    expect(view.result.current.recommendedWorkflow).toBe(first);
  });

  it("ignores a stale config response from the previous project", async () => {
    let resolveA!: (value: ProjectWorkflowConfigView) => void;
    let resolveB!: (value: ProjectWorkflowConfigView) => void;
    mocks.getProjectWorkflowConfig
      .mockImplementationOnce(() => new Promise<ProjectWorkflowConfigView>((resolve) => { resolveA = resolve; }))
      .mockImplementationOnce(() => new Promise<ProjectWorkflowConfigView>((resolve) => { resolveB = resolve; }));
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options(),
    });
    view.rerender(options({ projectId: "project-b" }));
    await act(async () => resolveA(config("project-a", imageBinding(first))));
    expect(view.result.current.projectWorkflowConfig).toBeUndefined();
    await act(async () => resolveB(config("project-b", imageBinding(second))));
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-b"));
    expect(view.result.current.projectDefaultWorkflow?.recipeId).toBe(second.recipeId);
  });

  it("prefers a valid Krea2 recipe for recommendation and falls back to the first catalog recipe", () => {
    const kera = kera2Recipe();
    const withKera = renderHook(() => useGenerationWorkflowSelectionController(options({ productCatalog: [first, kera] })));
    expect(withKera.result.current.recommendedWorkflow?.workflowId).toBe(KERA2_WORKFLOW_ID);
    withKera.unmount();
    const withoutKera = renderHook(() => useGenerationWorkflowSelectionController(options({ productCatalog: [first, second] })));
    expect(withoutKera.result.current.recommendedWorkflow).toBe(first);
  });

  it("resolves available, missing, and unavailable project defaults without changing the binding", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", imageBinding(second)));
    const available = renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(available.result.current.projectDefaultWorkflow).toBe(second));
    expect(available.result.current.staleProjectDefault).toBe(false);
    available.unmount();

    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", imageBinding(third)));
    const missing = renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(missing.result.current.staleProjectDefault).toBe(true));
    expect(missing.result.current.projectWorkflowConfig?.imageDefault?.recipeId).toBe(third.recipeId);
    missing.unmount();

    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", imageBinding(second, false)));
    const unavailable = renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(unavailable.result.current.staleProjectDefault).toBe(true));
    expect(unavailable.result.current.projectWorkflowConfig?.imageDefault?.available).toBe(false);
  });

  it("reconciles the store to the recommended workflow when no selection exists", async () => {
    renderHook(() => useGenerationWorkflowSelectionController(options()));
    await waitFor(() => expect(useStudioStore.getState().selectedWorkflow?.recipeId).toBe(first.recipeId));
  });

  it("selects an exact manual workflow and keeps selectedWorkflow in the Studio store", async () => {
    const changed = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    expect(useStudioStore.getState().selectedWorkflow?.recipeId).toBe(second.recipeId);
    expect(changed).toHaveBeenCalledTimes(1);
    view.rerender(options({ selectedWorkflow: second, onWorkflowChanged: changed }));
    expect(view.result.current.selectionSource).toBe("manual");
  });

  it("treats a same workflow as a no-op and compares version plus recipe exactly", async () => {
    const changed = vi.fn();
    const sameWorkflow = imageRecipe("same");
    const differentIdentity = imageRecipe("same", { workflowVersionId: "version-other" });
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ productCatalog: [sameWorkflow, differentIdentity], selectedWorkflow: sameWorkflow, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.selectWorkflow(sameWorkflow));
    expect(changed).not.toHaveBeenCalled();
    await act(async () => view.result.current.selectWorkflow(differentIdentity));
    expect(useStudioStore.getState().selectedWorkflow?.workflowVersionId).toBe("version-other");
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("keeps migration and confirmation behavior when switching a dirty draft", async () => {
    const changed = vi.fn();
    useStudioStore.getState().loadDraft(first, { prompt: { type: "string", value: "keep me" }, width: { type: "integer", value: 1024 } });
    useStudioStore.getState().setValue("prompt", { type: "string", value: "dirty" });
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, draftDirty: true, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    expect(window.confirm).toHaveBeenCalledWith("当前 Studio 草稿有未保存修改，确认切换工作流吗？");
    expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "dirty" });
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("does not switch or reset composition state when dirty confirmation is rejected", async () => {
    const changed = vi.fn();
    useStudioStore.getState().loadDraft(first, { prompt: { type: "string", value: "keep" } });
    vi.spyOn(window, "confirm").mockReturnValue(false);
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, draftDirty: true, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    expect(useStudioStore.getState().selectedWorkflow).toBe(first);
    expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "keep" });
    expect(changed).not.toHaveBeenCalled();
  });

  it("selects a workflow when there is no current store workflow", async () => {
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ productCatalog: [] }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    expect(useStudioStore.getState().selectedWorkflow).toBe(second);
  });

  it("restores the project default before the recommendation and clears manual selection", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValueOnce(config("project-a", imageBinding(second)));
    const changed = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onWorkflowChanged: changed }),
    });
    await waitFor(() => expect(view.result.current.projectDefaultWorkflow).toBe(second));
    await act(async () => view.result.current.restoreRecommendedWorkflow());
    expect(useStudioStore.getState().selectedWorkflow?.recipeId).toBe(second.recipeId);
    view.rerender(options({ selectedWorkflow: second, onWorkflowChanged: changed }));
    expect(view.result.current.selectionSource).toBe("project_default");
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("restoring the already selected fallback is a no-op", async () => {
    const changed = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.restoreRecommendedWorkflow());
    expect(changed).not.toHaveBeenCalled();
  });

  it("keeps the restore confirmation text and cancels when the dirty draft is rejected", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    const changed = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: second, draftDirty: true, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.restoreRecommendedWorkflow());
    expect(window.confirm).toHaveBeenCalledWith("当前 Studio 草稿有未保存修改，确认恢复推荐工作流吗？");
    expect(useStudioStore.getState().selectedWorkflow).toBeUndefined();
    expect(changed).not.toHaveBeenCalled();
  });

  it("recovers stale manual selection with the existing notice and fallback", async () => {
    const onNotice = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onNotice }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    view.rerender(options({ productCatalog: [first], selectedWorkflow: second, onNotice }));
    await waitFor(() => expect(onNotice).toHaveBeenCalledWith("本次手动选择的工作流当前不可用，已切换到项目默认或推荐工作流。"));
  });

  it("does not report stale manual selection for an empty catalog and recovers when it returns", async () => {
    const onNotice = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onNotice }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    view.rerender(options({ productCatalog: [], selectedWorkflow: second, onNotice }));
    expect(onNotice).not.toHaveBeenCalledWith("本次手动选择的工作流当前不可用，已切换到项目默认或推荐工作流。");
    view.rerender(options({ productCatalog: [first, second], selectedWorkflow: undefined, onNotice }));
    await waitFor(() => expect(useStudioStore.getState().selectedWorkflow?.recipeId).toBe(second.recipeId));
  });

  it("preserves an exact selected workflow during catalog refresh and reconciles a removed one", async () => {
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: second }),
    });
    view.rerender(options({ productCatalog: [first, second], selectedWorkflow: second }));
    expect(useStudioStore.getState().selectedWorkflow).toBeUndefined();
    view.rerender(options({ productCatalog: [first], selectedWorkflow: second }));
    await waitFor(() => expect(useStudioStore.getState().selectedWorkflow?.recipeId).toBe(first.recipeId));
  });

  it("continues a valid recent workflow and notices an unavailable one", async () => {
    const onNotice = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onNotice }),
    });
    await act(async () => view.result.current.continueRecentWorkflow({ workflowVersionId: "v", recipeId: "r", workflowName: "x", lastUsedAt: "now" }, second));
    expect(useStudioStore.getState().selectedWorkflow).toBe(second);
    await act(async () => view.result.current.continueRecentWorkflow({ workflowVersionId: "v", recipeId: "r", workflowName: "x", lastUsedAt: "now" }));
    expect(onNotice).toHaveBeenCalledWith("历史工作流当前不可用，无法创建新的创作入口。");
  });

  it("does not retain a manual selection across a project switch", async () => {
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    view.rerender(options({ projectId: "project-b", selectedWorkflow: second }));
    await waitFor(() => expect(view.result.current.projectWorkflowConfig?.projectId).toBe("project-b"));
    expect(view.result.current.selectionSource).not.toBe("manual");
  });

  it("invokes the cross-domain callback only for an actual workflow switch", async () => {
    const changed = vi.fn();
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first, onWorkflowChanged: changed }),
    });
    await act(async () => view.result.current.selectWorkflow(first));
    expect(changed).not.toHaveBeenCalled();
    await act(async () => view.result.current.selectWorkflow(second));
    expect(changed).toHaveBeenCalledTimes(1);
    view.rerender(options({ selectedWorkflow: second, onWorkflowChanged: changed }));
    await act(async () => view.result.current.restoreRecommendedWorkflow());
    expect(changed).toHaveBeenCalledTimes(2);
  });

  it("keeps the exact recipe reference used for manual selection", async () => {
    const view = renderHook((props: HookOptions) => useGenerationWorkflowSelectionController(props), {
      initialProps: options({ selectedWorkflow: first }),
    });
    await act(async () => view.result.current.selectWorkflow(second));
    view.rerender(options({ selectedWorkflow: second }));
    expect(view.result.current.manualWorkflow).toEqual(second);
    expect(recipeRef(second)).toEqual({ workflowVersionId: "version-second", recipeId: "recipe-second" });
  });
});
