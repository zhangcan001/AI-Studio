import { describe, expect, it } from "vitest";
import type { ShotStage, ShotView } from "../../types/shot";
import { bulkSelectionIds, shotHasStageConfig } from "./ShotBulkConfigPanel";

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

describe("Shot bulk selection", () => {
  const shots = [
    baseShot("sht-3", 2, { stageConfigs: [config("image"), config("video")] }),
    baseShot("sht-1", 0, { stageConfigs: [config("image")] }),
    baseShot("sht-2", 1),
  ];

  it("selects all shots in ordinal order", () => {
    expect(bulkSelectionIds(shots, "all", "image")).toEqual(["sht-1", "sht-2", "sht-3"]);
  });

  it("selects only current-stage Ready shots", () => {
    expect(bulkSelectionIds(shots, "ready", "image")).toEqual(["sht-1", "sht-3"]);
    expect(bulkSelectionIds(shots, "ready", "video")).toEqual(["sht-3"]);
  });

  it("selects current-stage unconfigured shots without changing the shot", () => {
    expect(bulkSelectionIds(shots, "unconfigured", "video")).toEqual(["sht-1", "sht-2"]);
    expect(shotHasStageConfig(shots[1], "image")).toBe(true);
    expect(shotHasStageConfig(shots[1], "video")).toBe(false);
  });
});
