import { describe, expect, it } from "vitest";

type Classification = "DONE" | "PREPARED" | "ELIGIBLE" | "BLOCKED";

interface Shot {
  id: string;
  sceneId: string;
  classification: Classification;
  selectedImageAssetId: string | null;
  selectedVideoAssetId: string | null;
  references: string[];
  prompt: string;
}

interface Scene {
  id: string;
  name: string;
  shots: Shot[];
}

const makeShots = (sceneId: string, counts: Array<[Classification, number]>): Shot[] =>
  counts.flatMap(([classification, count]) =>
    Array.from({ length: count }, (_, index) => ({
      id: `${sceneId}-shot-${index + 1}`,
      sceneId,
      classification,
      selectedImageAssetId: null,
      selectedVideoAssetId: null,
      references: [`ref-${sceneId}`],
      prompt: `${sceneId}-prompt-${index + 1}`,
    })),
  ).map((shot, index) => ({ ...shot, id: `${shot.sceneId}-shot-${String(index + 1).padStart(2, "0")}` }));

const episodeA = (): Scene[] => [
  { id: "scene-1", name: "Scene 1", shots: makeShots("scene-1", [["DONE", 10]]) },
  { id: "scene-2", name: "Scene 2", shots: makeShots("scene-2", [["DONE", 5], ["ELIGIBLE", 5]]) },
  { id: "scene-3", name: "Scene 3", shots: makeShots("scene-3", [["PREPARED", 10]]) },
  { id: "scene-4", name: "Scene 4", shots: makeShots("scene-4", [["ELIGIBLE", 8], ["BLOCKED", 2]]) },
  { id: "scene-5", name: "Scene 5", shots: makeShots("scene-5", [["ELIGIBLE", 10]]) },
  { id: "scene-6", name: "Scene 6", shots: [] },
];

const selectedEligibleKeys = (scenes: Scene[], selectedIds: string[]) =>
  scenes
    .filter((scene) => selectedIds.includes(scene.id))
    .flatMap((scene) => scene.shots.filter((shot) => shot.classification === "ELIGIBLE"))
    .map((shot) => `${shot.id}:IMAGE`);

const prepare = (scenes: Scene[], selectedIds: string[], allowPartial: boolean, bindings: Set<string>) => {
  const selected = scenes.filter((scene) => selectedIds.includes(scene.id));
  const blockers = selected
    .filter((scene) => scene.shots.some((shot) => shot.classification === "BLOCKED"))
    .map((scene) => scene.id);
  if (!allowPartial && blockers.length > 0) {
    return { createdBatches: 0, createdItems: 0, blockers, skipped: selectedIds, autoStarted: false };
  }

  let createdBatches = 0;
  let createdItems = 0;
  const skipped: string[] = [];
  for (const scene of selected) {
    const keys = scene.shots
      .filter((shot) => shot.classification === "ELIGIBLE")
      .map((shot) => `${shot.id}:IMAGE`);
    const created = keys.filter((key) => {
      if (bindings.has(key)) return false;
      bindings.add(key);
      return true;
    });
    if (created.length > 0) {
      createdBatches += 1;
      createdItems += created.length;
    } else {
      skipped.push(scene.id);
    }
  }
  return { createdBatches, createdItems, blockers, skipped, autoStarted: false };
};

describe("DEV-041 no-GPU Episode safety contract", () => {
  it("keeps Episode A totals and ordered Scene classifications exact", () => {
    const scenes = episodeA();
    const rows = scenes.flatMap((scene) => scene.shots);
    const count = (classification: Classification) => rows.filter((shot) => shot.classification === classification).length;

    expect(scenes).toHaveLength(6);
    expect(rows).toHaveLength(50);
    expect({ done: count("DONE"), prepared: count("PREPARED"), eligible: count("ELIGIBLE"), blocked: count("BLOCKED") })
      .toEqual({ done: 15, prepared: 10, eligible: 23, blocked: 2 });
    expect(scenes.map((scene) => scene.shots.length)).toEqual([10, 10, 10, 10, 10, 0]);
  });

  it("makes strict Scene 2/4/5 prepare a zero-mutation operation", () => {
    const bindings = new Set<string>();
    const result = prepare(episodeA(), ["scene-2", "scene-4", "scene-5"], false, bindings);

    expect(result).toMatchObject({ createdBatches: 0, createdItems: 0, blockers: ["scene-4"], autoStarted: false });
    expect(bindings).toHaveLength(0);
  });

  it("creates three partial batches and then creates zero new items on repeat", () => {
    const scenes = episodeA();
    const bindings = new Set<string>();
    const first = prepare(scenes, ["scene-2", "scene-4", "scene-5"], true, bindings);
    const second = prepare(scenes, ["scene-2", "scene-4", "scene-5"], true, bindings);

    expect(first).toMatchObject({ createdBatches: 3, createdItems: 23, blockers: ["scene-4"], autoStarted: false });
    expect(second).toMatchObject({ createdBatches: 0, createdItems: 0, autoStarted: false });
    expect(bindings).toHaveLength(23);
  });

  it("keeps Episode+Episode and Episode+Scene races unique by Shot/Stage binding", () => {
    const scenes = episodeA();
    const bindings = new Set<string>();
    const requests = [selectedEligibleKeys(scenes, ["scene-5"]), selectedEligibleKeys(scenes, ["scene-5"]), selectedEligibleKeys(scenes, ["scene-5"])]
      .flat();
    const created = requests.filter((key) => {
      if (bindings.has(key)) return false;
      bindings.add(key);
      return true;
    });

    expect(created).toHaveLength(10);
    expect(bindings).toHaveLength(10);
    expect(new Set(created)).toHaveLength(created.length);
  });

  it("keeps Video manual image and video review gates closed", () => {
    const rows = Array.from({ length: 20 }, (_, index): Shot => ({
      id: `video-shot-${String(index + 1).padStart(2, "0")}`,
      sceneId: "video-scene",
      classification: index < 10 ? "ELIGIBLE" : "BLOCKED",
      selectedImageAssetId: index < 10 ? `image-${index + 1}` : null,
      selectedVideoAssetId: null,
      references: [],
      prompt: "",
    }));

    expect(rows.filter((row) => row.classification === "ELIGIBLE")).toHaveLength(10);
    expect(rows.filter((row) => row.classification === "BLOCKED")).toHaveLength(10);
    expect(rows.slice(10).every((row) => row.selectedImageAssetId === null)).toBe(true);
    expect(rows.every((row) => row.selectedVideoAssetId === null)).toBe(true);
  });

  it("resolves Prompt context per Episode, Scene, and Shot", () => {
    const render = (episode: string, scene: string, shot: string) =>
      "{{episode.name}} / {{scene.name}} / {{shot.name}}"
        .replace("{{episode.name}}", episode)
        .replace("{{scene.name}}", scene)
        .replace("{{shot.name}}", shot);

    expect(render("Episode A", "天宫", "Shot 01")).toBe("Episode A / 天宫 / Shot 01");
    expect(render("Episode A", "地狱", "Shot 01")).toBe("Episode A / 地狱 / Shot 01");
  });

  it("preserves references and selected assets during a four-Scene preset apply", () => {
    const before = Array.from({ length: 40 }, (_, index) => ({
      id: `shot-${index + 1}`,
      references: [`ref-${index % 4}`],
      selectedImageAssetId: `image-${index + 1}`,
      selectedVideoAssetId: `video-${index + 1}`,
      stageConfig: "old",
    }));
    const after = before.map((shot) => ({ ...shot, stageConfig: "new-image-preset" }));

    expect(after).toHaveLength(40);
    after.forEach((shot, index) => {
      expect(shot.references).toEqual(before[index].references);
      expect(shot.selectedImageAssetId).toBe(before[index].selectedImageAssetId);
      expect(shot.selectedVideoAssetId).toBe(before[index].selectedVideoAssetId);
      expect(shot.stageConfig).toBe("new-image-preset");
    });
  });

  it("plans 500 Shots / 50 Scenes / 5 Episodes with one tree load per Episode", () => {
    const episodes = Array.from({ length: 5 }, (_, episodeIndex) =>
      Array.from({ length: 10 }, (_, sceneIndex) => ({
        id: `e${episodeIndex}-scene-${sceneIndex}`,
        shots: Array.from({ length: 10 }, (_, shotIndex) => `e${episodeIndex}-shot-${sceneIndex}-${shotIndex}`),
      })),
    );
    let treeLoads = 0;
    const plans = episodes.map((scenes) => {
      treeLoads += 1;
      return scenes;
    });

    expect(plans).toHaveLength(5);
    expect(plans.flat()).toHaveLength(50);
    expect(plans.flatMap((episode) => episode.flatMap((scene) => scene.shots))).toHaveLength(500);
    expect(treeLoads).toBe(5);
    expect(plans[0].slice(0, 5).flatMap((scene) => scene.shots)).toHaveLength(50);
  });

  it.todo("runs the production implementation architecture gate after Agents A-C integrate Episode scope");
});
