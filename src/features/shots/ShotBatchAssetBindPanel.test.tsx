// @vitest-environment jsdom
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotView } from "../../types/shot";
import { ShotBatchAssetBindPanel } from "./ShotBatchAssetBindPanel";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  inputsGet: vi.fn(),
  inputsSave: vi.fn(),
  invalidate: vi.fn(),
}));

vi.mock("../../product/client", () => ({
  productClient: {
    library: {
      list: (...args: unknown[]) => api.list(...args),
      get: (...args: unknown[]) => api.get(...args),
    },
    creation: {
      inputsGet: (...args: unknown[]) => api.inputsGet(...args),
      inputsSave: (...args: unknown[]) => api.inputsSave(...args),
    },
  },
}));
vi.mock("../../product/videoInputInvalidation", () => ({
  invalidateVideoInputs: (...args: unknown[]) => api.invalidate(...args),
  subscribeVideoInputInvalidation: () => () => undefined,
  resetVideoInputInvalidationForTests: () => undefined,
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const i2v: RecipeViewModel = {
  workflowId: "wf",
  workflowVersionId: "wfv-1",
  recipeId: "i2v",
  name: "图生视频",
  category: "video",
  mode: "I2V",
  persistentInputs: true,
  selectionRef: "opaque-i2v",
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "first_frame", type: "image", label: "首帧", required: true },
  ],
};

const fl: RecipeViewModel = {
  ...i2v,
  recipeId: "fl",
  name: "首尾帧",
  mode: "FIRST_LAST",
  selectionRef: "opaque-fl",
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "first_frame", type: "image", label: "首帧", required: true },
    { key: "last_frame", type: "image", label: "尾帧", required: true },
  ],
};

function shot(
  id: string,
  ordinal: number,
  recipeId: string,
): ShotView {
  return {
    id,
    projectId: "p1",
    ordinal,
    name: `镜头${ordinal + 1}`,
    promptText: "move",
    createdAt: "",
    updatedAt: "",
    status: "DRAFT",
    imageStatus: "IDLE",
    videoStatus: "IDLE",
    stageConfigs: [
      {
        stage: "video",
        workflowVersionId: "wfv-1",
        recipeId,
        scalarValues: {},
        updatedAt: "",
      },
    ],
    referenceAssets: [],
    generationLinks: [],
  };
}

beforeEach(() => {
  api.list.mockResolvedValue({
    items: [
      {
        resourceRef: { kind: "asset", id: "ext-1" },
        title: "外图一",
        subtype: "image",
        createdAt: "",
        updatedAt: "",
        thumbnailAvailable: false,
      },
      {
        resourceRef: { kind: "asset", id: "ext-2" },
        title: "外图二",
        subtype: "image",
        createdAt: "",
        updatedAt: "",
        thumbnailAvailable: false,
      },
    ],
    nextCursor: null,
    coverage: "keyset-page",
    coverageMessage: "page",
  });
  api.get.mockImplementation(async (_project: string, resource: { id: string }) => ({
    kind: "asset",
    asset: {
      id: resource.id,
      category: "source_image",
      name: resource.id,
      originalName: resource.id,
      mimeType: "image/png",
      fileSize: 1,
      createdAt: "",
      isFavorite: false,
      tags: [],
      assetType: "image",
    },
  }));
  api.inputsGet.mockImplementation(async (selection: { selectionRef: string }) => ({
    selectionRef: selection.selectionRef,
    token: { instanceId: "tok", revision: 1 },
    inputs: [],
    assets: [],
  }));
  api.inputsSave.mockImplementation(async (request: { selection: { selectionRef: string }; inputs: unknown[] }) => ({
    selectionRef: request.selection.selectionRef,
    token: { instanceId: "tok", revision: 2 },
    inputs: request.inputs,
    assets: [],
  }));
});

describe("ShotBatchAssetBindPanel", () => {
  it("matrix path OCC-saves first_frame for multiple shots without generate/queue", async () => {
    const user = userEvent.setup();
    const onBound = vi.fn();
    render(
      <ShotBatchAssetBindPanel
        projectId="p1"
        shots={[shot("s1", 0, "i2v"), shot("s2", 1, "fl")]}
        catalog={[i2v, fl]}
        onBound={onBound}
      />,
    );
    await screen.findByRole("table", { name: "多镜头正式槽绑定矩阵" });
    await waitFor(() => expect(api.list).toHaveBeenCalled());
    expect(api.list.mock.calls[0][1]).toMatchObject({ sourceKind: "source" });

    await user.selectOptions(
      screen.getByLabelText("镜头 1 首帧"),
      "ext-1",
    );
    await user.selectOptions(
      screen.getByLabelText("镜头 2 首帧"),
      "ext-2",
    );
    await user.selectOptions(
      screen.getByLabelText("镜头 2 尾帧"),
      "ext-1",
    );
    await user.click(screen.getByRole("button", { name: "写入正式输入" }));

    await waitFor(() => expect(api.inputsSave).toHaveBeenCalledTimes(2));
    expect(api.inputsSave.mock.calls[0][0]).toMatchObject({
      selection: { projectId: "p1", shotId: "s1", selectionRef: "opaque-i2v" },
      expected: { instanceId: "tok", revision: 1 },
      inputs: [{ inputKey: "first_frame", ordinal: 0, assetId: "ext-1" }],
    });
    expect(api.inputsSave.mock.calls[1][0].inputs).toEqual([
      { inputKey: "first_frame", ordinal: 0, assetId: "ext-2" },
      { inputKey: "last_frame", ordinal: 0, assetId: "ext-1" },
    ]);
    expect(await screen.findByText(/已写入 2 个镜头的正式视频输入/)).toBeTruthy();
    expect(onBound).toHaveBeenCalled();
    expect(api.invalidate).toHaveBeenCalledWith("p1");
    // No second queue / generate surface in this panel.
    expect(screen.queryByRole("button", { name: /开始生产|生成/ })).toBeNull();
  });

  it("TSV path checks then writes and rejects generated image categories", async () => {
    const user = userEvent.setup();
    api.get.mockResolvedValueOnce({
      kind: "asset",
      asset: {
        id: "gen-1",
        category: "generated_image",
        name: "gen",
        originalName: "gen",
        mimeType: "image/png",
        fileSize: 1,
        createdAt: "",
        isFavorite: false,
        tags: [],
        assetType: "image",
        sourceTaskId: "task",
      },
    });
    render(
      <ShotBatchAssetBindPanel
        projectId="p1"
        shots={[shot("s1", 0, "i2v")]}
        catalog={[i2v]}
      />,
    );
    await user.click(screen.getByRole("tab", { name: "表格文本" }));
    await user.type(
      screen.getByLabelText("批量绑定表格内容"),
      "1\tfirst_frame\tgen-1",
    );
    await user.click(screen.getByRole("button", { name: "检查" }));
    const preview = await screen.findByRole("table", { name: "批量绑定预览" });
    expect(within(preview).getByText("可写入")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "确认写入" }));
    await waitFor(() =>
      expect(screen.getByText(/只接受外部导入素材/)).toBeTruthy(),
    );
    expect(api.inputsSave).not.toHaveBeenCalled();
  });
});
