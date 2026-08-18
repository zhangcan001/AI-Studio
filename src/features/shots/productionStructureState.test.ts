import { describe, expect, it } from "vitest";
import type { ProductionStructureTree } from "../../types/productionStructure";
import {
  findProductionSceneParent,
  moveOrderedId,
  normalizeStructureName,
  productionSceneOptions,
  shotSceneIndex,
} from "./productionStructureState";

const tree: ProductionStructureTree = {
  projectId: "project-1",
  unassignedShotIds: ["shot-3"],
  series: [{
    id: "series-1",
    projectId: "project-1",
    ordinal: 0,
    name: "第一季",
    description: "",
    createdAt: "",
    updatedAt: "",
    episodes: [{
      id: "episode-1",
      seriesId: "series-1",
      ordinal: 0,
      name: "第一集",
      description: "",
      createdAt: "",
      updatedAt: "",
      scenes: [{
        id: "scene-1",
        episodeId: "episode-1",
        ordinal: 0,
        name: "入口",
        description: "",
        shotIds: ["shot-1", "shot-2"],
        createdAt: "",
        updatedAt: "",
      }],
    }],
  }],
};

describe("production structure frontend state", () => {
  it("builds deterministic scene options and a shot-to-scene index", () => {
    expect(productionSceneOptions(tree)).toEqual([
      { value: "ALL", label: "全部镜头" },
      { value: "UNASSIGNED", label: "未归档（1）" },
      { value: "scene-1", label: "S01 / E01 / 入口" },
    ]);
    expect(shotSceneIndex(tree)).toEqual({ "shot-1": "scene-1", "shot-2": "scene-1" });
    expect(findProductionSceneParent(tree, "scene-1")?.episode.id).toBe("episode-1");
  });

  it("moves only local order and fails closed at boundaries", () => {
    expect(moveOrderedId(["a", "b", "c"], 1, -1)).toEqual(["b", "a", "c"]);
    expect(moveOrderedId(["a", "b", "c"], 0, -1)).toEqual(["a", "b", "c"]);
    expect(moveOrderedId(["a", "b", "c"], 4, 1)).toEqual(["a", "b", "c"]);
  });

  it("normalizes names to one line and the UI length limit", () => {
    expect(normalizeStructureName("  第一\n季  ")).toBe("第一 季");
    expect(normalizeStructureName("   ")).toBeUndefined();
    expect(normalizeStructureName("x".repeat(120))).toHaveLength(100);
  });
});
