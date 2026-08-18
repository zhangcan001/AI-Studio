import { describe, expect, it } from "vitest";
import type { ShotStage, ShotView } from "../../types/shot";
import {
  deriveProjectPipelineSummary,
  projectCompletionPercent,
  projectPipelineStageCount,
  reviewShotIds,
} from "./ProjectProductionPipeline";
import { buildShotListView, defaultShotListControls } from "./shotListQuery";

const baseShot = (id: string, ordinal: number, overrides: Partial<ShotView> = {}): ShotView => ({
  id,
  projectId: "prj-1",
  ordinal,
  name: id,
  promptText: `prompt-${id}`,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  status: "DRAFT",
  imageStatus: "DRAFT",
  videoStatus: "DRAFT",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
  ...overrides,
});

function config(stage: ShotStage) {
  return {
    stage,
    workflowVersionId: `${stage}-workflow`,
    recipeId: `${stage}-recipe`,
    scalarValues: {},
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

function withTask(id: string, stage: ShotStage, status: string): Partial<ShotView> {
  return {
    generationLinks: [{
      id: `${id}-link`,
      stage,
      createdAt: "2026-01-01T00:00:00Z",
      task: { id: `${id}-task`, status, outputAssetIds: [] } as unknown as ShotView["generationLinks"][number]["task"],
    }],
  };
}

describe("Project production pipeline derivation", () => {
  it("derives every displayed stage from Shot state and preserves review gates", () => {
    const shots = [
      baseShot("unconfigured", 0),
      baseShot("image-ready", 1, { stageConfigs: [config("image"), config("video")] }),
      baseShot("image-generating", 2, { stageConfigs: [config("image"), config("video")], ...withTask("image-generating", "image", "RUNNING") }),
      baseShot("image-review", 3, { stageConfigs: [config("image"), config("video")], ...withTask("image-review", "image", "SUCCEEDED") }),
      baseShot("image-selected", 4, { stageConfigs: [config("image"), config("video")], selectedImageAssetId: "ast-image" }),
      baseShot("video-ready", 5, { stageConfigs: [config("image"), config("video")], selectedImageAssetId: "ast-image" }),
      baseShot("video-generating", 6, { stageConfigs: [config("image"), config("video")], selectedImageAssetId: "ast-image", ...withTask("video-generating", "video", "RUNNING") }),
      baseShot("video-review", 7, { stageConfigs: [config("image"), config("video")], selectedImageAssetId: "ast-image", ...withTask("video-review", "video", "SUCCEEDED") }),
      baseShot("completed", 8, { stageConfigs: [config("image"), config("video")], selectedImageAssetId: "ast-image", selectedVideoAssetId: "ast-video" }),
      baseShot("failed", 9, { stageConfigs: [config("image"), config("video")], ...withTask("failed", "video", "FAILED") }),
    ];

    const summary = deriveProjectPipelineSummary(shots);
    expect(summary.total).toBe(10);
    expect(summary.unconfigured).toBe(1);
    expect(summary.imageConfigured).toBe(9);
    expect(summary.imageReady).toBe(2);
    expect(summary.imageGenerating).toBe(1);
    expect(summary.imageReview).toBe(1);
    expect(summary.imageSelected).toBe(5);
    expect(summary.videoConfigured).toBe(9);
    expect(summary.videoReady).toBe(5);
    expect(summary.videoGenerating).toBe(1);
    expect(summary.videoReview).toBe(1);
    expect(summary.completed).toBe(1);
    expect(summary.failed).toBe(1);
  });

  it("reports completion and review ids without selecting any asset", () => {
    const shots = [
      baseShot("video-review-2", 2, { stageConfigs: [config("video")], ...withTask("video-review-2", "video", "SUCCEEDED") }),
      baseShot("image-review-1", 1, { stageConfigs: [config("image")], ...withTask("image-review-1", "image", "SUCCEEDED") }),
    ];
    const summary = deriveProjectPipelineSummary(shots);

    expect(projectCompletionPercent(summary)).toBe(0);
    expect(reviewShotIds(shots, "image")).toEqual(["image-review-1"]);
    expect(reviewShotIds(shots, "video")).toEqual(["video-review-2"]);
    expect(projectPipelineStageCount(summary, "IMAGE_REVIEW")).toBe(1);
    expect(projectPipelineStageCount(summary, "VIDEO_REVIEW")).toBe(1);
    expect(shots.every((shot) => !shot.selectedImageAssetId && !shot.selectedVideoAssetId)).toBe(true);
  });

  it("uses zero percent for an empty project", () => {
    const summary = deriveProjectPipelineSummary([]);
    expect(summary.total).toBe(0);
    expect(projectCompletionPercent(summary)).toBe(0);
    expect(projectPipelineStageCount(summary, "SHOTS")).toBe(0);
  });

  it("keeps full-project pipeline totals when the workspace list is filtered", () => {
    const shots = Array.from({ length: 500 }, (_, index) => baseShot(`shot-${index + 1}`, index));
    const listView = buildShotListView(shots, { ...defaultShotListControls(), query: "shot-499" });

    expect(listView.pageShots).toHaveLength(1);
    expect(deriveProjectPipelineSummary(shots).total).toBe(500);
  });
});
