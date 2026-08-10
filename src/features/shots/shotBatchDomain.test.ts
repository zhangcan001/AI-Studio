import { describe, expect, it } from "vitest";
import type { ShotView } from "../../types/shot";
import { shotProgressSummary } from "./shotBatchDomain";

const base = (overrides: Partial<ShotView> = {}): ShotView => ({
  id: "sht-1",
  projectId: "project-1",
  ordinal: 0,
  name: "镜头",
  promptText: "一只猫",
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

describe("shot batch progress", () => {
  it("counts selected keyframes and completion without treating a failed retry as primary", () => {
    const shots = [
      base({ selectedImageAssetId: "ast-image" }),
      base({ id: "sht-2", ordinal: 1, stageConfigs: [{ stage: "video", workflowVersionId: "wfv", recipeId: "h3", scalarValues: {}, updatedAt: "2026-01-01T00:00:00Z" }], videoStatus: "GENERATING_VIDEO" }),
    ];
    const summary = shotProgressSummary(shots);
    expect(summary).toMatchObject({ total: 2, keyframesSelected: 1, completed: 1, videoGenerating: 0 });
  });
});
