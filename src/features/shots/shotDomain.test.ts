import { describe, expect, it } from "vitest";
import { deriveShotStatus, deriveStageStatus, isExecutionTruthStatus } from "./shotDomain";
import type { ShotView } from "../../types/shot";

function shot(overrides: Partial<ShotView> = {}): ShotView {
  return {
    id: "sht-1",
    projectId: "project-1",
    ordinal: 0,
    name: "开场",
    promptText: "wide",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    status: "DRAFT",
    imageStatus: "DRAFT",
    videoStatus: "DRAFT",
    stageConfigs: [],
    referenceAssets: [],
    generationLinks: [],
    ...overrides,
  };
}

describe("Pack 09 shot status derivation", () => {
  it("derives active and review states from linked task truth", () => {
    const configured = shot({
      stageConfigs: [{ stage: "image", workflowVersionId: "wfv-1", recipeId: "img", scalarValues: {}, updatedAt: "2026-01-01T00:00:00Z" }],
      generationLinks: [{ id: "lnk-1", stage: "image", taskId: "tsk-1", createdAt: "2026-01-01T00:00:00Z", task: {
        id: "tsk-1", projectId: "project-1", status: "RUNNING", progress: { mode: "indeterminate" }, createdAt: "2026-01-01T00:00:00Z", outputAssetIds: [],
      } }],
    });
    expect(deriveStageStatus(configured, "image")).toBe("GENERATING_IMAGE");
    expect(deriveShotStatus(configured)).toBe("GENERATING_IMAGE");
    expect(deriveStageStatus({ ...configured, generationLinks: [{ ...configured.generationLinks[0], task: { ...configured.generationLinks[0].task!, status: "SUCCEEDED" } }] }, "image")).toBe("IMAGE_REVIEW");
  });

  it("never treats persisted task execution statuses as Shot database states", () => {
    expect(isExecutionTruthStatus("RUNNING")).toBe(true);
    expect(isExecutionTruthStatus("FAILED")).toBe(true);
    expect(isExecutionTruthStatus("COMPLETED")).toBe(false);
    expect(deriveShotStatus(shot({ selectedImageAssetId: "ast-image" }))).toBe("COMPLETED");
  });
});
