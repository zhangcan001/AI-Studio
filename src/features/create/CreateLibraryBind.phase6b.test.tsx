// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationContext, GeneratorOption } from "../../product/types";

const api = vi.hoisted(() => ({
  get: vi.fn(), generatorsList: vi.fn(), inputsGet: vi.fn(), inputsSave: vi.fn(),
  assetsImport: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), libraryGet: vi.fn(),
}));
vi.mock("../../product/client", () => ({
  productClient: {
    creation: {
      get: (...a: unknown[]) => api.get(...a),
      generatorsList: (...a: unknown[]) => api.generatorsList(...a),
      inputsGet: (...a: unknown[]) => api.inputsGet(...a),
      inputsSave: (...a: unknown[]) => api.inputsSave(...a),
      assetsImport: (...a: unknown[]) => api.assetsImport(...a),
      readinessGet: (...a: unknown[]) => api.readinessGet(...a),
      generate: (...a: unknown[]) => api.generate(...a),
      mediaUrl: () => "http://fixture/media",
    },
    library: { get: (...a: unknown[]) => api.libraryGet(...a) },
    run: { get: vi.fn(), retry: vi.fn() },
  },
}));

const params = [
  { type: "integer" as const, key: "width", label: "Width", required: true, default: 640 },
  { type: "integer" as const, key: "height", label: "Height", required: true, default: 480 },
  { type: "integer" as const, key: "duration_seconds", label: "Duration", required: true, default: 5 },
];
const fl: GeneratorOption = {
  selectionRef: "exact-fl", name: "FL", mode: "FIRST_LAST", mediaKind: "video", version: "2.2",
  availability: true, availabilityReason: null, recommended: true, persistentInputs: true,
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    ...params,
    { key: "first_frame", type: "image", label: "首帧", required: true },
    { key: "last_frame", type: "image", label: "尾帧", required: true },
  ],
};
const ref2va: GeneratorOption = {
  ...fl, selectionRef: "exact-ref", name: "REF", mode: "REF2VA", recommended: false,
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    ...params,
    { key: "reference_images", type: "images", label: "参考图", required: false, minItems: 0, maxItems: 9 },
    { key: "reference_videos", type: "videos", label: "参考视频", required: false, minItems: 0, maxItems: 3 },
    { key: "reference_audios", type: "audios", label: "参考音频", required: false, minItems: 0, maxItems: 3 },
  ],
};
function ctx(selectionRef = "exact-fl"): CreationContext {
  return {
    projectId: "project-a", projectName: "A", stage: "video",
    shots: [{ id: "shot-a", name: "镜头一", ordinal: 0 }],
    selectedShot: {
      summary: { id: "shot-a", name: "镜头一", ordinal: 0 }, prompt: "默认", selectionRef,
      values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null,
    },
    candidates: [], mediaInputs: [], promptChoices: [],
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  useStudioStore.getState().resetDraft();
  api.get.mockResolvedValue(ctx());
  api.generatorsList.mockResolvedValue([fl, ref2va]);
  api.inputsGet.mockImplementation(async (s: { selectionRef: string }) => ({
    selectionRef: s.selectionRef, token: { instanceId: "i1", revision: 1 }, inputs: [], assets: [],
  }));
  api.inputsSave.mockImplementation(async (r: { selection: { selectionRef: string }; inputs: unknown[] }) => ({
    selectionRef: r.selection.selectionRef, token: { instanceId: "i1", revision: 2 }, inputs: r.inputs, assets: [],
  }));
  api.readinessGet.mockResolvedValue({ ready: false, issues: [], fieldErrors: [], actions: [] });
});
afterEach(() => { cleanup(); sessionStorage.clear(); });

it("B01/B02 external image binds to first_frame via OCC; generated image cannot use image slots", async () => {
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "ext-img", mediaKind: "image" });
  api.libraryGet.mockResolvedValue({ kind: "asset", asset: { id: "ext-img", projectId: "project-a", name: "外部图", assetType: "image", category: "source_image", width: 1, height: 1 } });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await screen.findByRole("button", { name: "应用到首帧" });
  await waitFor(() => {
    const btn = screen.getByRole("button", { name: "应用到首帧" });
    expect(btn.hasAttribute("disabled")).toBe(false);
  });
  fireEvent.click(screen.getByRole("button", { name: "应用到首帧" }));
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalled());
  expect(api.inputsSave.mock.calls[0][0].inputs).toEqual([{ inputKey: "first_frame", ordinal: 0, assetId: "ext-img" }]);
  expect(api.generate).not.toHaveBeenCalled();
  cleanup();

  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "gen-img", mediaKind: "image" });
  api.libraryGet.mockResolvedValue({ kind: "asset", asset: { id: "gen-img", projectId: "project-a", name: "生成图", assetType: "image", category: "generated_image", sourceTaskId: "tsk", width: 1, height: 1 } });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await screen.findByText(/不能再绑定为首帧/);
  expect(screen.queryByRole("button", { name: /应用到首帧/ })).toBeNull();
});

it("B02 generated video requires confirm and only binds reference_videos", async () => {
  api.get.mockResolvedValue(ctx("exact-ref"));
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "gen-vid", mediaKind: "video" });
  api.libraryGet.mockResolvedValue({ kind: "asset", asset: { id: "gen-vid", projectId: "project-a", name: "生成视频", assetType: "video", category: "generated_video", sourceTaskId: "tsk" } });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  const apply = await screen.findByRole("button", { name: "应用到参考视频" });
  expect(apply.hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: /确认将生成视频仅作为视频参考/ }));
  await waitFor(() => expect(screen.getByRole("button", { name: "应用到参考视频" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "应用到参考视频" }));
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalled());
  expect(api.inputsSave.mock.calls[api.inputsSave.mock.calls.length - 1][0].inputs).toEqual([{ inputKey: "reference_videos", ordinal: 0, assetId: "gen-vid" }]);
  expect(api.generate).not.toHaveBeenCalled();
});

it("B03 foreign project intent is dropped without save", async () => {
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "other", assetId: "ext-img", mediaKind: "image" });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await screen.findByLabelText("选择生成器");
  expect(useStudioStore.getState().pendingLibraryIntent).toBeUndefined();
  expect(api.inputsSave).not.toHaveBeenCalled();
});

it("Apply first_frame keeps unsaved dirty last_frame in OCC payload", async () => {
  api.inputsGet.mockImplementation(async (s: { selectionRef: string }) => ({
    selectionRef: s.selectionRef, token: { instanceId: "i1", revision: 1 },
    inputs: [
      { inputKey: "first_frame", ordinal: 0, assetId: "old-first" },
      { inputKey: "last_frame", ordinal: 0, assetId: "old-last" },
    ],
    assets: [],
  }));
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "ext-img", mediaKind: "image" });
  api.libraryGet.mockResolvedValue({ kind: "asset", asset: { id: "ext-img", projectId: "project-a", name: "外部图", assetType: "image", category: "source_image", width: 1, height: 1 } });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await screen.findByRole("button", { name: "应用到首帧" });
  await waitFor(() => expect(useStudioStore.getState().values.last_frame).toEqual({ type: "image_asset", assetId: "old-last" }));
  // Dirty sibling before Apply
  useStudioStore.getState().setValue("last_frame", { type: "image_asset", assetId: "dirty-last" });
  await waitFor(() => {
    const btn = screen.getByRole("button", { name: "应用到首帧" });
    expect(btn.hasAttribute("disabled")).toBe(false);
  });
  fireEvent.click(screen.getByRole("button", { name: "应用到首帧" }));
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalled());
  const saved = api.inputsSave.mock.calls[api.inputsSave.mock.calls.length - 1][0].inputs;
  expect(saved).toEqual(expect.arrayContaining([
    { inputKey: "first_frame", ordinal: 0, assetId: "ext-img" },
    { inputKey: "last_frame", ordinal: 0, assetId: "dirty-last" },
  ]));
  expect(useStudioStore.getState().values.last_frame).toEqual({ type: "image_asset", assetId: "dirty-last" });
});

it("unknown/empty category after library get failure blocks Apply into image slots", async () => {
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "mystery", mediaKind: "image" });
  api.libraryGet.mockRejectedValue({ code: "LIBRARY_RESOURCE_NOT_FOUND", message: "missing" });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await waitFor(() => expect(screen.getAllByText(/无法确认素材来源类别/).length).toBeGreaterThan(0));
  expect(screen.queryByRole("button", { name: /应用到首帧/ })).toBeNull();
  expect(api.inputsSave).not.toHaveBeenCalled();
  expect(useStudioStore.getState().values.first_frame).toBeUndefined();
});

it("empty category asset meta fails closed for image Apply", async () => {
  useStudioStore.getState().setPendingLibraryIntent({ kind: "asset", projectId: "project-a", assetId: "empty-cat", mediaKind: "image" });
  api.libraryGet.mockResolvedValue({ kind: "asset", asset: { id: "empty-cat", projectId: "project-a", name: "无类别", assetType: "image", category: "", width: 1, height: 1 } });
  render(<CreatePage route={{ kind: "create", projectId: "project-a", shotId: "shot-a", stage: "video" }} navigate={vi.fn()} />);
  await waitFor(() => expect(screen.getAllByText(/无法确认素材来源类别/).length).toBeGreaterThan(0));
  expect(screen.queryByRole("button", { name: /应用到首帧/ })).toBeNull();
  expect(api.inputsSave).not.toHaveBeenCalled();
});
