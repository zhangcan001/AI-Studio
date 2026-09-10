// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { PresetView } from "../../../types/preset";
import { useStudioStore } from "../../../stores/studioStore";
import { useGenerationPresetController } from "./useGenerationPresetController";

const mocks = vi.hoisted(() => ({
  listPresets: vi.fn(),
  getPreferredPreset: vi.fn(),
  setPreferredPreset: vi.fn(),
  createPreset: vi.fn(),
  updatePreset: vi.fn(),
  deletePreset: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    listPresets: mocks.listPresets,
    getPreferredPreset: mocks.getPreferredPreset,
    setPreferredPreset: mocks.setPreferredPreset,
    createPreset: mocks.createPreset,
    updatePreset: mocks.updatePreset,
    deletePreset: mocks.deletePreset,
  };
});

const workflow = (suffix = "a"): RecipeViewModel => ({
  workflowId: `workflow-${suffix}`,
  workflowVersionId: `workflow-version-${suffix}`,
  recipeId: `recipe-${suffix}`,
  name: `Workflow ${suffix}`,
  category: "image",
  mode: "text-to-image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
});

const values = (text: string): GenerationValues => ({
  prompt: { type: "string", value: text },
});

function preset(id = "preset-a", overrides: Partial<PresetView> = {}): PresetView {
  return {
    id,
    projectId: "project-a",
    workflowVersionId: "workflow-version-a",
    recipeId: "recipe-a",
    name: `Preset ${id}`,
    values: values(id),
    createdAt: "2026-09-10T00:00:00Z",
    updatedAt: "2026-09-10T00:00:00Z",
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const defaultWorkflow = workflow();

function options(overrides: Partial<Parameters<typeof useGenerationPresetController>[0]> = {}) {
  return {
    projectId: "project-a",
    selectedWorkflow: defaultWorkflow,
    ...overrides,
  };
}

describe("useGenerationPresetController", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    useStudioStore.getState().resetDraft();
    mocks.listPresets.mockReset();
    mocks.getPreferredPreset.mockReset();
    mocks.setPreferredPreset.mockReset();
    mocks.createPreset.mockReset();
    mocks.updatePreset.mockReset();
    mocks.deletePreset.mockReset();
    mocks.listPresets.mockResolvedValue([]);
    mocks.getPreferredPreset.mockResolvedValue(null);
    mocks.setPreferredPreset.mockResolvedValue(undefined);
    mocks.deletePreset.mockResolvedValue(undefined);
  });

  it("loads presets with the exact workflow version and recipe identity", async () => {
    renderHook(() => useGenerationPresetController(options()));

    await waitFor(() => expect(mocks.listPresets).toHaveBeenCalledTimes(1));
    expect(mocks.listPresets).toHaveBeenCalledWith("project-a", "workflow-version-a", "recipe-a");
    expect(mocks.getPreferredPreset).toHaveBeenCalledWith("project-a", "workflow-version-a", "recipe-a");
  });

  it("auto-applies a preferred preset only when the draft is clean", async () => {
    const preferred = preset();
    mocks.listPresets.mockResolvedValue([preferred]);
    mocks.getPreferredPreset.mockResolvedValue(preferred.id);
    const clean = renderHook(() => useGenerationPresetController(options()));

    await waitFor(() => expect(clean.result.current.selectedPresetId).toBe(preferred.id));
    expect(useStudioStore.getState().values).toEqual(preferred.values);
    expect(clean.result.current.presetName).toBe(preferred.name);
  });

  it("does not overwrite a dirty draft with the preferred preset", async () => {
    const currentWorkflow = workflow();
    const preferred = preset();
    useStudioStore.getState().loadDraft(currentWorkflow, values("user draft"));
    useStudioStore.getState().setValue("prompt", { type: "string", value: "edited draft" });
    mocks.listPresets.mockResolvedValue([preferred]);
    mocks.getPreferredPreset.mockResolvedValue(preferred.id);
    const { result } = renderHook(() => useGenerationPresetController(options({ selectedWorkflow: currentWorkflow })));

    await waitFor(() => expect(result.current.presets).toHaveLength(1));
    expect(result.current.selectedPresetId).toBe("");
    expect(useStudioStore.getState().values).toEqual(values("edited draft"));
  });

  it("ignores stale and unmounted loading completions", async () => {
    const oldRequest = deferred<PresetView[]>();
    const oldWorkflow = workflow("old");
    const nextWorkflow = workflow("next");
    const unmountedWorkflow = workflow("unmounted");
    mocks.listPresets.mockImplementationOnce(() => oldRequest.promise).mockResolvedValueOnce([preset("next-preset", {
      projectId: "project-a",
      workflowVersionId: nextWorkflow.workflowVersionId,
      recipeId: nextWorkflow.recipeId,
    })]);
    const { result, rerender, unmount } = renderHook(
      (props: Parameters<typeof useGenerationPresetController>[0]) => useGenerationPresetController(props),
      { initialProps: options({ selectedWorkflow: oldWorkflow }) },
    );

    await waitFor(() => expect(mocks.listPresets).toHaveBeenCalledTimes(1));
    rerender(options({ selectedWorkflow: nextWorkflow }));
    await waitFor(() => expect(result.current.presets[0]?.id).toBe("next-preset"));
    await act(async () => { oldRequest.resolve([preset("old-preset")]); });
    expect(result.current.presets[0]?.id).toBe("next-preset");

    const pending = deferred<PresetView[]>();
    mocks.listPresets.mockImplementationOnce(() => pending.promise);
    const unmounted = renderHook(() => useGenerationPresetController(options({ selectedWorkflow: unmountedWorkflow })));
    await waitFor(() => expect(mocks.listPresets).toHaveBeenCalledTimes(3));
    unmounted.unmount();
    await act(async () => { pending.resolve([preset("unmounted-result")]); });
    unmount();
  });

  it("applies exact values, updates selection, and clears missing asset state through the callback", async () => {
    const onPresetApplied = vi.fn();
    const applied = preset("exact", { name: "Exact preset", values: values("applied") });
    const { result } = renderHook(() => useGenerationPresetController(options({ onPresetApplied })));

    await waitFor(() => expect(result.current.presetLoading).toBe(false));
    act(() => result.current.applyPreset(applied));
    expect(useStudioStore.getState().values).toEqual(applied.values);
    expect(result.current.selectedPresetId).toBe(applied.id);
    expect(result.current.presetName).toBe(applied.name);
    expect(result.current.presetEditorOpen).toBe(false);
    expect(onPresetApplied).toHaveBeenCalledTimes(1);
  });

  it("blocks saving an empty preset name", async () => {
    const { result } = renderHook(() => useGenerationPresetController(options()));

    await waitFor(() => expect(result.current.presetLoading).toBe(false));
    await act(async () => { await result.current.savePreset(); });
    expect(mocks.createPreset).not.toHaveBeenCalled();
    expect(result.current.presetError).toContain("请输入预设名称");
  });

  it("creates a preset with the exact project, workflow, recipe, name, and values", async () => {
    const created = preset("created", { name: "Created", values: values("created values") });
    mocks.createPreset.mockResolvedValue(created);
    const { result } = renderHook(() => useGenerationPresetController(options()));

    await waitFor(() => expect(result.current.presetLoading).toBe(false));
    act(() => result.current.setPresetName("Created"));
    await act(async () => { await result.current.savePreset(); });
    expect(mocks.createPreset).toHaveBeenCalledWith({
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      name: "Created",
      values: {},
    });
    expect(result.current.selectedPresetId).toBe(created.id);
  });

  it("updates the selected preset and falls back to create when no preset is selected", async () => {
    const selected = preset("selected");
    const updated = preset("selected", { name: "Updated" });
    mocks.listPresets.mockResolvedValue([selected]);
    mocks.updatePreset.mockResolvedValue(updated);
    const { result } = renderHook(() => useGenerationPresetController(options()));
    await waitFor(() => expect(result.current.presets).toHaveLength(1));
    act(() => result.current.applyPreset(selected));
    act(() => result.current.setPresetName("Updated"));
    await act(async () => { await result.current.savePresetChanges(); });
    expect(mocks.updatePreset).toHaveBeenCalledWith({
      projectId: "project-a",
      presetId: "selected",
      name: "Updated",
      values: selected.values,
    });

    mocks.createPreset.mockResolvedValue(preset("fallback"));
    act(() => result.current.clearSelection());
    act(() => result.current.setPresetName("Fallback"));
    await act(async () => { await result.current.savePresetChanges(); });
    expect(mocks.createPreset).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      name: "Fallback",
    }));
  });

  it("requires confirmation before deleting a preset", async () => {
    const selected = preset("selected");
    mocks.listPresets.mockResolvedValue([selected]);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { result } = renderHook(() => useGenerationPresetController(options()));
    await waitFor(() => expect(result.current.presets).toHaveLength(1));
    act(() => result.current.applyPreset(selected));
    await act(async () => { await result.current.removePreset(); });
    expect(confirm).toHaveBeenCalledWith("确定删除这个预设吗？");
    expect(mocks.deletePreset).not.toHaveBeenCalled();
  });

  it("clears a preferred preset before deleting it with exact identities", async () => {
    const selected = preset("selected");
    mocks.listPresets.mockResolvedValue([selected]);
    mocks.getPreferredPreset.mockResolvedValue(selected.id);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const { result } = renderHook(() => useGenerationPresetController(options()));
    await waitFor(() => expect(result.current.selectedPresetId).toBe(selected.id));
    await act(async () => { await result.current.removePreset(); });
    expect(mocks.setPreferredPreset).toHaveBeenCalledWith({
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
    });
    expect(mocks.deletePreset).toHaveBeenCalledWith("project-a", selected.id);
    expect(mocks.setPreferredPreset.mock.invocationCallOrder[0]).toBeLessThan(mocks.deletePreset.mock.invocationCallOrder[0]);
    expect(result.current.selectedPresetId).toBe("");
  });

  it("toggles the preferred preset using the exact workflow identity and clears it on the second toggle", async () => {
    const selected = preset("selected");
    mocks.listPresets.mockResolvedValue([selected]);
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationPresetController(options({ onNotice })));
    await waitFor(() => expect(result.current.presetLoading).toBe(false));
    act(() => result.current.applyPreset(selected));
    await act(async () => { await result.current.togglePreferredPreset(); });
    expect(mocks.setPreferredPreset).toHaveBeenNthCalledWith(1, {
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      presetId: selected.id,
    });
    expect(result.current.preferredPresetId).toBe(selected.id);
    await act(async () => { await result.current.togglePreferredPreset(); });
    expect(mocks.setPreferredPreset).toHaveBeenNthCalledWith(2, {
      projectId: "project-a",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      presetId: undefined,
    });
    expect(result.current.preferredPresetId).toBeNull();
    expect(onNotice).toHaveBeenCalledTimes(2);
  });

  it("resets only controller preset state when project or workflow changes", async () => {
    const first = preset("first");
    const secondWorkflow = workflow("second");
    const second = preset("second", {
      projectId: "project-b",
      workflowVersionId: secondWorkflow.workflowVersionId,
      recipeId: secondWorkflow.recipeId,
    });
    mocks.listPresets.mockResolvedValueOnce([first]).mockResolvedValueOnce([second]);
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationPresetController>[0]) => useGenerationPresetController(props),
      { initialProps: options() },
    );
    await waitFor(() => expect(result.current.presets).toHaveLength(1));
    act(() => result.current.applyPreset(first));
    act(() => result.current.openEditor());
    rerender(options({ projectId: "project-b", selectedWorkflow: secondWorkflow }));
    await waitFor(() => expect(result.current.presets[0]?.id).toBe(second.id));
    expect(result.current.selectedPresetId).toBe("");
    expect(result.current.preferredPresetId).toBeNull();
    expect(result.current.presetName).toBe("");
    expect(result.current.presetError).toBeUndefined();
    expect(result.current.presetEditorOpen).toBe(false);
  });
});
