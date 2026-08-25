import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  createBatchWorkflowPreset,
  deleteBatchWorkflowPreset,
  getSceneProductionPlan,
  listBatchWorkflowPresets,
  prepareSceneProduction,
  startProductionQueue,
  updateBatchWorkflowPreset,
} from "../../services/tauriClient";
import type { BatchWorkflowPreset, SceneProductionPlan } from "../../types/sceneProduction";
import { sanitizeReusableGenerationValues } from "../../types/sceneProduction";
import { SceneProductionPanel, presetOverwriteConfirmation, sceneProductionActionDisabled } from "./SceneProductionPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const preset: BatchWorkflowPreset = {
  id: "bwp_1",
  name: "电影基础",
  description: "可跨项目复用",
  image: { workflowVersionId: "wf_1", recipeId: "recipe_image", values: { steps: { type: "integer", value: 24 } } },
  video: { workflowVersionId: "wf_2", recipeId: "recipe_video", values: { fps: { type: "integer", value: 24 } } },
  createdAt: "",
  updatedAt: "",
  available: true,
};

const plan: SceneProductionPlan = {
  projectId: "project-1",
  sceneId: "scene-1",
  sceneName: "入口",
  stage: "image",
  total: 4,
  done: 1,
  prepared: 1,
  eligible: 1,
  blocked: 1,
  canPrepare: true,
  maxBatchItems: 100,
  rows: [
    { shotId: "shot-1", name: "镜头 01", globalOrdinal: 0, classification: "DONE", blockingReasons: [] },
    { shotId: "shot-2", name: "镜头 02", globalOrdinal: 1, classification: "PREPARED", blockingReasons: [], existingBatchId: "batch-1" },
    { shotId: "shot-3", name: "镜头 03", globalOrdinal: 2, classification: "ELIGIBLE", blockingReasons: [] },
    { shotId: "shot-4", name: "镜头 04", globalOrdinal: 3, classification: "BLOCKED", blockingReasons: ["缺少 Workflow"] },
  ],
};

describe("SceneProductionPanel", () => {
  it("renders scene/stage selectors, preset status, prompt actions, plan counts, rows, and safe prepare actions", () => {
    const html = renderToStaticMarkup(
      <SceneProductionPanel
        projectId="project-1"
        sceneOptions={[{ value: "scene-1", label: "S01 / E01 / 入口" }]}
        currentSceneId="scene-1"
        initialPresets={[preset]}
        initialPlan={plan}
        promptEntries={[{
          id: "prompt-1",
          projectId: "project-1",
          kind: "prompt",
          name: "电影提示词",
          tags: [],
          createdAt: "",
          updatedAt: "",
          versionCount: 1,
          versions: [{ id: "prompt-version-1", promptId: "prompt-1", version: 1, text: "{{shot.name}}", createdAt: "" }],
        }]}
      />,
    );

    expect(html).toContain("场景选择");
    expect(html).toContain("生产阶段");
    expect(html).toContain("电影基础");
    expect(html).toContain("应用图片预设到场景");
    expect(html).toContain("提示词条目");
    expect(html).toContain("应用图片提示词");
    expect(html).toContain("已完成");
    expect(html).toContain("已准备");
    expect(html).toContain("可生产");
    expect(html).toContain("被阻塞");
    expect(html).toContain("缺少 Workflow");
    expect(html).toContain("仅准备当前可生产镜头");
    expect(html).toContain("启动生产");
  });

  it("keeps preset snapshots free of media assets and confirms overwrite scope", () => {
    const values = sanitizeReusableGenerationValues({
      steps: { type: "integer", value: 24 },
      reference: { type: "image_assets", assetIds: ["asset-1"] },
      selectedVideo: { type: "video_asset", assetId: "video-1" },
    });
    expect(values).toEqual({ steps: { type: "integer", value: 24 } });
    expect(presetOverwriteConfirmation("image", 12)).toContain("覆盖场景内 12 个镜头的图片阶段配置");
    expect(presetOverwriteConfirmation("image", 12)).toContain("引用素材和已选媒体不会改变");
    expect(sceneProductionActionDisabled("prepare")).toBe(true);
    expect(sceneProductionActionDisabled(undefined)).toBe(false);
  });
});

describe("Scene Production client contracts", () => {
  it("uses the backend command envelopes and reuses the existing queue batch id", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([preset]);
    await listBatchWorkflowPresets();
    expect(invoke).toHaveBeenLastCalledWith("batch_workflow_presets_list");

    vi.mocked(invoke).mockResolvedValueOnce(preset);
    await createBatchWorkflowPreset({ name: "新预设", image: preset.image });
    expect(invoke).toHaveBeenLastCalledWith("batch_workflow_preset_create", { input: { name: "新预设", image: preset.image } });

    vi.mocked(invoke).mockResolvedValueOnce(preset);
    await updateBatchWorkflowPreset({ presetId: preset.id, name: "重命名", image: preset.image });
    expect(invoke).toHaveBeenLastCalledWith("batch_workflow_preset_update", { presetId: preset.id, input: { name: "重命名", image: preset.image } });

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await deleteBatchWorkflowPreset(preset.id);
    expect(invoke).toHaveBeenLastCalledWith("batch_workflow_preset_delete", { presetId: preset.id });

    vi.mocked(invoke).mockResolvedValueOnce(plan);
    await getSceneProductionPlan({ projectId: "project-1", sceneId: "scene-1", stage: "image" });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_plan", { projectId: "project-1", sceneId: "scene-1", stage: "image" });

    vi.mocked(invoke).mockResolvedValueOnce({
      projectId: "project-1",
      sceneId: "scene-1",
      stage: "image",
      created: true,
      createdCount: 1,
      alreadyPrepared: false,
      existingBatchIds: [],
      detail: { id: "batch-1" },
    });
    const prepared = await prepareSceneProduction({ projectId: "project-1", sceneId: "scene-1", stage: "image", allowPartial: false });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_prepare", { request: { projectId: "project-1", sceneId: "scene-1", stage: "image", allowPartial: false } });
    expect(prepared.batchId).toBe("batch-1");

    vi.mocked(invoke).mockResolvedValueOnce({ id: "batch-1" });
    await startProductionQueue("project-1", "batch-1");
    expect(invoke).toHaveBeenLastCalledWith("production_queue_start", { projectId: "project-1", batchId: "batch-1" });
  });
});
