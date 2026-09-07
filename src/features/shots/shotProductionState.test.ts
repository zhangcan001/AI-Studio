import { describe, expect, it } from "vitest";
import type { ShotGenerationLink, ShotStage, ShotView } from "../../types/shot";
import { buildShotProductionReadModel, type ShotProductionContext } from "./shotProductionState";

function shot(overrides: Partial<ShotView> = {}): ShotView {
  return {
    id: "shot-1",
    projectId: "project-1",
    ordinal: 0,
    name: "开场",
    promptText: "wide cinematic shot",
    createdAt: "2026-09-07T00:00:00Z",
    updatedAt: "2026-09-07T00:00:00Z",
    status: "DRAFT",
    imageStatus: "DRAFT",
    videoStatus: "DRAFT",
    stageConfigs: [],
    referenceAssets: [],
    generationLinks: [],
    ...overrides,
  };
}

function config(stage: ShotStage) {
  return {
    stage,
    workflowVersionId: `${stage}-workflow-version`,
    recipeId: `${stage}-recipe`,
    scalarValues: {},
    updatedAt: "2026-09-07T00:00:00Z",
  } as const;
}

function taskLink(stage: ShotStage, status: "RUNNING" | "SUCCEEDED" | "FAILED"): ShotGenerationLink {
  return {
    id: `${stage}-link`,
    stage,
    taskId: `${stage}-task`,
    createdAt: "2026-09-07T00:00:00Z",
    task: {
      id: `${stage}-task`,
      projectId: "project-1",
      status,
      progress: { mode: "indeterminate" },
      createdAt: "2026-09-07T00:00:00Z",
      outputAssetIds: status === "SUCCEEDED" ? [`${stage}-asset`] : [],
    },
  };
}

function context(overrides: ShotProductionContext = {}): ShotProductionContext {
  return {
    image: { available: true, configured: true },
    video: { available: true, configured: true, videoInputMode: "TEXT_ONLY" },
    ...overrides,
  };
}

function step(model: ReturnType<typeof buildShotProductionReadModel>, id: string) {
  return model.steps.find((item) => item.id === id);
}

describe("shot production read model", () => {
  it("keeps optional references complete and routes the next action to image generation", () => {
    const model = buildShotProductionReadModel(shot(), context());

    expect(step(model, "prompt")?.status).toBe("COMPLETE");
    expect(step(model, "references")?.status).toBe("COMPLETE");
    expect(model.nextAction).toMatchObject({ stepId: "image", actionable: true });
  });

  it("blocks only the required reference stage until its minimum is met", () => {
    const required = context({ image: { available: true, configured: true, referenceRequired: true, referenceMinimum: 2, referenceCount: 1 } });
    const blocked = buildShotProductionReadModel(shot(), required);
    expect(step(blocked, "references")).toMatchObject({ status: "BLOCKED", detail: "还需要 1 张参考素材" });
    expect(blocked.nextAction.stepId).toBe("references");

    const ready = buildShotProductionReadModel(shot(), context({ image: { ...required.image, referenceCount: 2 } }));
    expect(step(ready, "references")?.status).toBe("COMPLETE");
  });

  it("maps linked execution truth to active, review, and failure steps", () => {
    const generating = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image")], generationLinks: [taskLink("image", "RUNNING")] }),
      context(),
    );
    expect(step(generating, "image")).toMatchObject({ status: "ACTIVE", detail: "生成任务进行中" });
    expect(generating.nextAction.stepId).toBe("image");

    const reviewing = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image")], generationLinks: [taskLink("image", "SUCCEEDED")] }),
      context(),
    );
    expect(step(reviewing, "image")?.status).toBe("COMPLETE");
    expect(step(reviewing, "image-review")).toMatchObject({ status: "ACTIVE", detail: "候选等待人工确认" });
    expect(reviewing.nextAction.stepId).toBe("image-review");

    const failed = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image")], generationLinks: [taskLink("image", "FAILED")] }),
      context(),
    );
    expect(step(failed, "image")).toMatchObject({ status: "FAILED" });
  });

  it("requires a selected keyframe for single-image video and skips an unconfigured video after image completion", () => {
    const singleImage = context({ video: { available: true, configured: true, videoInputMode: "SINGLE_IMAGE" } });
    const blocked = buildShotProductionReadModel(shot({ stageConfigs: [config("image"), config("video")] }), singleImage);
    expect(step(blocked, "video")).toMatchObject({ status: "BLOCKED", detail: "请先选择关键帧图片" });

    const imageOnlyComplete = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image")], selectedImageAssetId: "image-1" }),
      { image: { available: true, configured: true }, video: { available: false, configured: false } },
    );
    expect(step(imageOnlyComplete, "video")?.status).toBe("COMPLETE");
    expect(step(imageOnlyComplete, "video-review")?.status).toBe("COMPLETE");
    expect(imageOnlyComplete.nextAction.stepId).toBe("complete");
  });

  it("moves video candidates from generation to review and completion", () => {
    const generating = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image"), config("video")], selectedImageAssetId: "image-1", generationLinks: [taskLink("video", "RUNNING")] }),
      context({ video: { available: true, configured: true, videoInputMode: "SINGLE_IMAGE" } }),
    );
    expect(step(generating, "video")?.status).toBe("ACTIVE");
    expect(generating.nextAction.stepId).toBe("video");

    const reviewing = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image"), config("video")], selectedImageAssetId: "image-1", generationLinks: [taskLink("video", "SUCCEEDED")] }),
      context({ video: { available: true, configured: true, videoInputMode: "SINGLE_IMAGE" } }),
    );
    expect(step(reviewing, "video")?.status).toBe("COMPLETE");
    expect(step(reviewing, "video-review")?.status).toBe("ACTIVE");
    expect(reviewing.nextAction.stepId).toBe("video-review");

    const complete = buildShotProductionReadModel(
      shot({ stageConfigs: [config("image"), config("video")], selectedImageAssetId: "image-1", selectedVideoAssetId: "video-1" }),
      context({ video: { available: true, configured: true, videoInputMode: "SINGLE_IMAGE" } }),
    );
    expect(complete.steps.every((item) => item.status === "COMPLETE")).toBe(true);
    expect(complete.nextAction).toMatchObject({ stepId: "complete", actionable: false });
  });
});
