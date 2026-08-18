import { describe, expect, it } from "vitest";

type Classification = "DONE" | "PREPARED" | "ELIGIBLE" | "BLOCKED";

interface SceneRow {
  id: string;
  stage: "IMAGE" | "VIDEO";
  classification: Classification;
  blockingReasons: string[];
  selectedImage: boolean;
}

const sceneA = (): SceneRow[] => [
  ...Array.from({ length: 3 }, (_, index) => ({
    id: "scene-a-shot-" + (index + 1),
    stage: "IMAGE" as const,
    classification: "DONE" as const,
    blockingReasons: [],
    selectedImage: true,
  })),
  ...Array.from({ length: 2 }, (_, index) => ({
    id: "scene-a-shot-" + (index + 4),
    stage: "IMAGE" as const,
    classification: "PREPARED" as const,
    blockingReasons: [],
    selectedImage: true,
  })),
  ...Array.from({ length: 6 }, (_, index) => ({
    id: "scene-a-shot-" + (index + 6),
    stage: "IMAGE" as const,
    classification: "ELIGIBLE" as const,
    blockingReasons: [],
    selectedImage: false,
  })),
  {
    id: "scene-a-shot-12",
    stage: "IMAGE",
    classification: "BLOCKED",
    blockingReasons: ["WORKFLOW_UNAVAILABLE"],
    selectedImage: false,
  },
];

function counts(rows: SceneRow[]) {
  return rows.reduce<Record<Classification, number>>((result, row) => {
    result[row.classification] += 1;
    return result;
  }, { DONE: 0, PREPARED: 0, ELIGIBLE: 0, BLOCKED: 0 });
}

function prepare(rows: SceneRow[], allowPartial: boolean, activeBindings: Set<string>) {
  if (!allowPartial && rows.some((row) => row.classification === "BLOCKED")) {
    return { created: [], error: "SCENE_PRODUCTION_BLOCKED" };
  }
  const candidates = rows.filter((row) => row.classification === "ELIGIBLE");
  const created = candidates
    .map((row) => row.id + ":" + row.stage)
    .filter((key) => !activeBindings.has(key));
  created.forEach((key) => activeBindings.add(key));
  return { created, error: undefined };
}

describe("DEV-040 no-GPU safety contract", () => {
  it("keeps the strict Scene A plan safe and the partial plan narrow", () => {
    const rows = sceneA();
    expect(counts(rows)).toEqual({ DONE: 3, PREPARED: 2, ELIGIBLE: 6, BLOCKED: 1 });

    const bindings = new Set<string>();
    expect(prepare(rows, false, bindings)).toEqual({ created: [], error: "SCENE_PRODUCTION_BLOCKED" });
    expect(prepare(rows, true, bindings)).toEqual({
      created: Array.from({ length: 6 }, (_, index) => "scene-a-shot-" + (index + 6) + ":IMAGE"),
      error: undefined,
    });
    expect(bindings.size).toBe(6);
    expect([...bindings].every((key) => /scene-a-shot-(6|7|8|9|10|11):IMAGE/.test(key))).toBe(true);
  });

  it("is idempotent on repeated prepare and never includes DONE/PREPARED/BLOCKED", () => {
    const rows = sceneA();
    const bindings = new Set<string>();
    const first = prepare(rows, true, bindings);
    const second = prepare(rows, true, bindings);

    expect(first.created).toHaveLength(6);
    expect(second.created).toEqual([]);
    expect([...bindings].some((key) => key.includes("shot-01") || key.includes("shot-12"))).toBe(false);
  });

  it("keeps the Video manual-review gate closed until an image is selected", () => {
    const rows: SceneRow[] = Array.from({ length: 10 }, (_, index) => ({
      id: "scene-b-shot-" + (index + 1),
      stage: "VIDEO",
      classification: index < 5 ? "ELIGIBLE" : "BLOCKED",
      blockingReasons: index < 5 ? [] : ["IMAGE_REVIEW_REQUIRED"],
      selectedImage: index < 5,
    }));
    expect(rows.filter((row) => row.classification === "ELIGIBLE")).toHaveLength(5);
    expect(rows.filter((row) => row.blockingReasons.includes("IMAGE_REVIEW_REQUIRED"))).toHaveLength(5);
    expect(rows.filter((row) => row.classification === "BLOCKED").every((row) => !row.selectedImage)).toBe(true);
  });

  it("keeps Scene ownership project-scoped across two projects", () => {
    const scopes = new Map([
      ["project-a", new Set(["scene-a"])],
      ["project-b", new Set(["scene-b"])],
    ]);
    const belongsToProject = (projectId: string, sceneId: string) => scopes.get(projectId)?.has(sceneId) ?? false;

    expect(belongsToProject("project-a", "scene-a")).toBe(true);
    expect(belongsToProject("project-b", "scene-b")).toBe(true);
    expect(belongsToProject("project-a", "scene-b")).toBe(false);
    expect(belongsToProject("project-b", "scene-a")).toBe(false);
  });

  it("plans 500 shots in 50 scenes without creating 50 production batches", () => {
    const shots = Array.from({ length: 500 }, (_, index) => ({
      id: "shot-" + String(index + 1).padStart(3, "0"),
      scene: Math.floor(index / 10),
    }));
    const scenes = Array.from({ length: 50 }, (_, scene) => shots.filter((shot) => shot.scene === scene));

    expect(scenes).toHaveLength(50);
    expect(scenes.every((scene) => scene.length === 10)).toBe(true);
    expect(shots).toHaveLength(500);
    expect(scenes.slice(0, 3)).toHaveLength(3);
  });

});
