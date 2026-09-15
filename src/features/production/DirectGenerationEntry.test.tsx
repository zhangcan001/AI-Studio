// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelVersionView, ModelView } from "../../types/model";
import type { PromptEntryView } from "../../types/prompt";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotView } from "../../types/shot";
import type { ToolInstanceView, ToolVersionView, ToolView } from "../../types/tool";
import { DirectGenerationEntry } from "./DirectGenerationEntry";

const mocks = vi.hoisted(() => ({
  createProductionQueue: vi.fn(),
  listPromptLibrary: vi.fn(),
  listModels: vi.fn(),
  listModelVersions: vi.fn(),
  listTools: vi.fn(),
  listToolInstances: vi.fn(),
  listToolVersions: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return {
    ...actual,
    createProductionQueue: mocks.createProductionQueue,
    listPromptLibrary: mocks.listPromptLibrary,
    listModels: mocks.listModels,
    listModelVersions: mocks.listModelVersions,
    listTools: mocks.listTools,
    listToolInstances: mocks.listToolInstances,
    listToolVersions: mocks.listToolVersions,
  };
});

const recipe: RecipeViewModel = {
  workflowId: "workflow-image",
  workflowVersionId: "workflow-version-image",
  recipeId: "recipe-image",
  name: "Image Recipe",
  category: "image",
  mode: "text_to_image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "a lantern" }],
  outputTypes: ["image"],
};

const shot: ShotView = {
  id: "shot-1",
  projectId: "project-1",
  ordinal: 0,
  name: "雨夜巷口",
  promptText: "a lantern",
  createdAt: "2026-09-15T00:00:00Z",
  updatedAt: "2026-09-15T00:00:00Z",
  status: "DRAFT",
  imageStatus: "DRAFT",
  videoStatus: "DRAFT",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
};

const prompt: PromptEntryView = {
  id: "prompt-1",
  projectId: "project-1",
  kind: "prompt",
  name: "雨夜提示词",
  tags: [],
  createdAt: "2026-09-15T00:00:00Z",
  updatedAt: "2026-09-15T00:00:00Z",
  versionCount: 1,
  versions: [{ id: "prompt-version-1", promptId: "prompt-1", version: 1, text: "a lantern", createdAt: "2026-09-15T00:00:00Z" }],
};

const model: ModelView = {
  id: "model-1",
  name: "MiniMax H3",
  provider: "MiniMax",
  type: "video",
  description: "H3",
  metadata: {},
  createdAt: "2026-09-15T00:00:00Z",
};

const modelVersion: ModelVersionView = {
  id: "model-version-1",
  modelId: "model-1",
  version: "1.0",
  capabilities: ["image"],
  parameterSchema: {},
  createdAt: "2026-09-15T00:00:00Z",
};

const tool: ToolView = {
  id: "tool-1",
  name: "ComfyUI",
  type: "image_generation",
  description: "Local ComfyUI",
  metadata: {},
  createdAt: "2026-09-15T00:00:00Z",
};

const instance: ToolInstanceView = {
  id: "tool-instance-1",
  toolId: "tool-1",
  endpoint: "http://127.0.0.1:8188",
  status: "AVAILABLE",
};

const toolVersion: ToolVersionView = {
  id: "tool-version-1",
  toolId: "tool-1",
  version: "0.3",
  observedAt: "2026-09-15T00:00:00Z",
  metadata: {},
};

const createdBatch: ProductionBatchDetail = {
  id: "batch-direct-1",
  projectId: "project-1",
  name: "单次生成",
  status: "READY",
  continueOnFailure: true,
  createdAt: "2026-09-15T00:00:00Z",
  updatedAt: "2026-09-15T00:00:00Z",
  total: 1,
  pending: 1,
  running: 0,
  succeeded: 0,
  failed: 0,
  cancelled: 0,
  skipped: 0,
  items: [{
    id: "batch-item-direct-1",
    ordinal: 0,
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    status: "PENDING",
  }],
};

beforeEach(() => {
  mocks.createProductionQueue.mockResolvedValue(createdBatch);
  mocks.listPromptLibrary.mockResolvedValue({ items: [prompt] });
  mocks.listModels.mockResolvedValue([model]);
  mocks.listModelVersions.mockResolvedValue([modelVersion]);
  mocks.listTools.mockResolvedValue([tool]);
  mocks.listToolInstances.mockResolvedValue([instance]);
  mocks.listToolVersions.mockResolvedValue([toolVersion]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

function renderEntry(overrides: Partial<ComponentProps<typeof DirectGenerationEntry>> = {}) {
  return render(<DirectGenerationEntry projectId="project-1" projectName="个人项目" catalog={[recipe]} shots={[shot]} {...overrides} />);
}

describe("DirectGenerationEntry", () => {
  it("creates a no-Shot generation as one waiting queue item", async () => {
    const user = userEvent.setup();
    renderEntry();

    await user.click(await screen.findByRole("button", { name: "创建单次生成" }));

    await waitFor(() => expect(mocks.createProductionQueue).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-1",
      direct: true,
      shotId: undefined,
      stage: undefined,
      items: [{
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        values: { prompt: { type: "string", value: "a lantern" } },
      }],
    })));
    expect(screen.getByText(/当前待启动/)).toBeTruthy();
    expect(screen.getByText(/生产队列；点击“开始生产”前不会创建任务/)).toBeTruthy();
  });

  it("passes exact Shot and provenance selections without starting the queue", async () => {
    const user = userEvent.setup();
    renderEntry({ onOpenProductionQueue: vi.fn() });

    await user.selectOptions(await screen.findByLabelText("目标"), shot.id);
    await user.selectOptions(screen.getByLabelText("阶段"), "video");
    await user.selectOptions(await screen.findByLabelText("提示词版本 可选"), "prompt-version-1");
    await user.selectOptions(await screen.findByLabelText("模型版本 可选"), "model-version-1");
    await user.selectOptions(await screen.findByLabelText("工具实例 可选"), "tool-instance-1");
    await user.selectOptions(screen.getByLabelText("工具版本 需先选择实例"), "tool-version-1");
    await user.click(screen.getByRole("button", { name: "创建单次生成" }));

    await waitFor(() => expect(mocks.createProductionQueue).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "project-1",
      direct: true,
      shotId: shot.id,
      stage: "video",
      promptVersionId: "prompt-version-1",
      modelVersionId: "model-version-1",
      toolInstanceId: "tool-instance-1",
      toolVersionId: "tool-version-1",
    })));
    expect(screen.getByText(/请在队列中明确点击“开始生产”/)).toBeTruthy();
  });

  it("shows disabled and empty states without creating a queue", () => {
    renderEntry({ enabled: false, catalog: [], shots: [] });
    expect(screen.getByText(/切换到此页后/)).toBeTruthy();
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
  });
});
