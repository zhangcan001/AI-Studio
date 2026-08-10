import { describe, expect, it } from "vitest";
import { reorderShots, selectShotResult, validateShotRecord, type ShotRecord } from "./shotDomain";

const shots: ShotRecord[] = [
  { id: "shot-1", projectId: "project-1", ordinal: 0, name: "开场", inlinePrompt: "wide", referenceAssetIds: [], status: "READY" },
  { id: "shot-2", projectId: "project-1", ordinal: 1, name: "特写", promptRef: "prm-1", workflowVersionId: "wfv-1", recipeId: "rcp-1", referenceAssetIds: [], status: "DRAFT" },
  { id: "shot-other", projectId: "project-2", ordinal: 0, name: "其他项目", referenceAssetIds: [], status: "DRAFT" },
];

describe("Pack 09 shot source slice", () => {
  it("keeps reorder project-scoped and assigns contiguous ordinals", () => {
    const reordered = reorderShots("project-1", shots, ["shot-2", "shot-1"]);
    expect(reordered.map((shot) => [shot.id, shot.ordinal])).toEqual([["shot-2", 0], ["shot-1", 1]]);
    expect(reorderShots("project-1", shots, ["shot-other", "shot-1"])).toHaveLength(2);
  });

  it("validates prompt/workflow shape and selects a result without creating a task", () => {
    expect(validateShotRecord(shots[0])).toEqual([]);
    expect(validateShotRecord({ ...shots[0], promptRef: "prm-1", inlinePrompt: "inline" })).toContain("prompt");
    expect(selectShotResult(shots[0], "asset-result")?.status).toBe("COMPLETED");
  });
});
