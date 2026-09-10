// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationValues, RecipeField, RecipeViewModel } from "../../../types/generation";
import { useStudioStore } from "../../../stores/studioStore";
import { useGenerationAssetIntentController } from "./useGenerationAssetIntentController";

const workflow = (fields: RecipeField[], suffix = "a"): RecipeViewModel => ({
  workflowId: `workflow-${suffix}`,
  workflowVersionId: `workflow-version-${suffix}`,
  recipeId: `recipe-${suffix}`,
  name: `Workflow ${suffix}`,
  category: "image",
  mode: "text-to-image",
  fields,
});

const singleImageField: RecipeField = {
  key: "reference",
  type: "image",
  label: "参考图",
  required: false,
};

const secondImageField: RecipeField = {
  key: "lastFrame",
  type: "image",
  label: "尾帧",
  required: false,
};

const multiImageField: RecipeField = {
  key: "references",
  type: "images",
  label: "参考图组",
  required: false,
  minItems: 0,
  maxItems: 2,
};

const values = (overrides: GenerationValues = {}): GenerationValues => ({
  prompt: { type: "string", value: "draft" },
  ...overrides,
});

function options(overrides: Partial<Parameters<typeof useGenerationAssetIntentController>[0]> = {}) {
  return {
    projectId: "project-a",
    selectedWorkflow: workflow([singleImageField]),
    onNotice: vi.fn(),
    onAssetFieldResolved: vi.fn(),
    ...overrides,
  };
}

function setIntent(projectId = "project-a", assetId = "asset-a", assetType: "image" | "video" | "audio" = "image") {
  useStudioStore.getState().setPendingAssetIntent({ projectId, assetId, assetType });
}

describe("useGenerationAssetIntentController", () => {
  beforeEach(() => {
    useStudioStore.getState().resetDraft();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("does not apply without a pending intent", () => {
    const onNotice = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({ onNotice })));

    expect(useStudioStore.getState().values).toEqual({});
    expect(onNotice).not.toHaveBeenCalled();
  });

  it("does not apply without a selected workflow and keeps the store intent", async () => {
    setIntent();
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: undefined,
      onNotice,
    })));

    await waitFor(() => expect(result.current.assetIntentTargets).toEqual([]));
    expect(useStudioStore.getState().pendingAssetIntent).toEqual({
      projectId: "project-a",
      assetId: "asset-a",
      assetType: "image",
    });
    expect(onNotice).not.toHaveBeenCalled();
  });

  it("clears a foreign project intent and does not touch the current draft", async () => {
    const currentValues = values({ reference: { type: "image_asset", assetId: "current" } });
    useStudioStore.getState().loadDraft(workflow([singleImageField]), currentValues);
    setIntent("project-b");
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({ onNotice })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(result.current.assetIntentTargets).toEqual([]);
    expect(useStudioStore.getState().values).toEqual(currentValues);
    expect(onNotice).toHaveBeenCalledWith("素材属于其他项目，已取消使用。");
  });

  it("clears the intent when the workflow has no compatible target", async () => {
    setIntent();
    const onNotice = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: workflow([{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "" }]),
      onNotice,
    })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(onNotice).toHaveBeenCalledWith("当前工作流没有可使用此素材的输入项。");
  });

  it("automatically applies the only compatible field and resolves missing state", async () => {
    setIntent();
    const onAssetFieldResolved = vi.fn();
    const onNotice = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({ onAssetFieldResolved, onNotice })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(useStudioStore.getState().values.reference).toEqual({ type: "image_asset", assetId: "asset-a" });
    expect(onAssetFieldResolved).toHaveBeenCalledWith("reference");
    expect(onNotice).toHaveBeenCalledWith("已将素材加入创作。");
  });

  it("exposes multiple compatible fields without guessing a target", async () => {
    setIntent();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: workflow([singleImageField, secondImageField]),
    })));

    await waitFor(() => expect(result.current.assetIntentTargets.map((field) => field.key)).toEqual(["reference", "lastFrame"]));
    expect(useStudioStore.getState().pendingAssetIntent).toBeDefined();
    expect(useStudioStore.getState().values).toEqual({});
  });

  it("applies the manually selected target and clears the intent", async () => {
    setIntent();
    const onAssetFieldResolved = vi.fn();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: workflow([singleImageField, secondImageField]),
      onAssetFieldResolved,
    })));

    await waitFor(() => expect(result.current.assetIntentTargets).toHaveLength(2));
    act(() => result.current.applyToTarget(result.current.assetIntentTargets[1]));

    expect(useStudioStore.getState().values.lastFrame).toEqual({ type: "image_asset", assetId: "asset-a" });
    expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined();
    expect(onAssetFieldResolved).toHaveBeenCalledWith("lastFrame");
  });

  it("cancels a multi-target intent without changing the draft", async () => {
    setIntent();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: workflow([singleImageField, secondImageField]),
    })));

    await waitFor(() => expect(result.current.assetIntentTargets).toHaveLength(2));
    act(() => result.current.cancel());

    expect(result.current.assetIntentTargets).toEqual([]);
    expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined();
    expect(useStudioStore.getState().values).toEqual({});
  });

  it("accepts replacement confirmation and applies exactly once", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    useStudioStore.getState().loadDraft(workflow([singleImageField]), values({
      reference: { type: "image_asset", assetId: "old" },
    }));
    setIntent();
    const onAssetFieldResolved = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({ onAssetFieldResolved })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(useStudioStore.getState().values.reference).toEqual({ type: "image_asset", assetId: "asset-a" });
    expect(onAssetFieldResolved).toHaveBeenCalledTimes(1);
  });

  it("rejects replacement confirmation without clearing the intent", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    useStudioStore.getState().loadDraft(workflow([singleImageField]), values({
      reference: { type: "image_asset", assetId: "old" },
    }));
    setIntent();
    const onAssetFieldResolved = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({ onAssetFieldResolved })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeDefined());
    expect(useStudioStore.getState().values.reference).toEqual({ type: "image_asset", assetId: "old" });
    expect(onAssetFieldResolved).not.toHaveBeenCalled();
  });

  it("handles max_items with the existing notice and clears the intent", async () => {
    const limitedField: RecipeField = { ...multiImageField, maxItems: 1 };
    useStudioStore.getState().loadDraft(workflow([limitedField]), values({
      references: { type: "image_assets", assetIds: ["old"] },
    }));
    setIntent();
    const onNotice = vi.fn();
    renderHook(() => useGenerationAssetIntentController(options({
      selectedWorkflow: workflow([limitedField]),
      onNotice,
    })));

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(useStudioStore.getState().values.references).toEqual({ type: "image_assets", assetIds: ["old"] });
    expect(onNotice).toHaveBeenCalledWith("“参考图组”已达到素材数量上限。");
  });

  it("surfaces a generic non-applied result when an explicit target is incompatible", async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationAssetIntentController(options({ onNotice })));
    const textField: RecipeField = { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" };

    act(() => {
      setIntent("project-a", "asset-a", "video");
      result.current.applyToTarget(textField);
    });
    expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined();
    expect(onNotice).toHaveBeenCalledWith("当前工作流没有可使用此素材的输入项。");
  });

  it("recomputes targets after a workflow switch and never applies a stale field", async () => {
    setIntent();
    const oldWorkflow = workflow([singleImageField, secondImageField], "old");
    const nextWorkflow = workflow([{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "" }], "next");
    const onNotice = vi.fn();
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationAssetIntentController>[0]) => useGenerationAssetIntentController(props),
      { initialProps: options({ selectedWorkflow: oldWorkflow, onNotice }) },
    );

    await waitFor(() => expect(result.current.assetIntentTargets).toHaveLength(2));
    rerender(options({ selectedWorkflow: nextWorkflow, onNotice }));
    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(result.current.assetIntentTargets).toEqual([]);
    expect(useStudioStore.getState().values).toEqual({});
    expect(onNotice).toHaveBeenCalledWith("当前工作流没有可使用此素材的输入项。");
  });

  it("replaces the target list when the workflow changes to a different compatible shape", async () => {
    setIntent();
    const oldWorkflow = workflow([singleImageField, secondImageField], "old");
    const nextWorkflow = workflow([multiImageField, secondImageField], "next");
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationAssetIntentController>[0]) => useGenerationAssetIntentController(props),
      { initialProps: options({ selectedWorkflow: oldWorkflow }) },
    );

    await waitFor(() => expect(result.current.assetIntentTargets).toHaveLength(2));
    rerender(options({ selectedWorkflow: nextWorkflow }));
    await waitFor(() => expect(result.current.assetIntentTargets.map((field) => field.key)).toEqual(["references", "lastFrame"]));
    expect(useStudioStore.getState().pendingAssetIntent).toBeDefined();
  });

  it("clears project-local target state when the project changes", async () => {
    setIntent();
    const onNotice = vi.fn();
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationAssetIntentController>[0]) => useGenerationAssetIntentController(props),
      { initialProps: options({ onNotice }) },
    );

    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    setIntent("project-a");
    rerender(options({ projectId: "project-b", onNotice }));
    await waitFor(() => expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined());
    expect(result.current.assetIntentTargets).toEqual([]);
    expect(onNotice).toHaveBeenCalledWith("素材属于其他项目，已取消使用。");
  });
});
