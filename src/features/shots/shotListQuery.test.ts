import { describe, expect, it } from "vitest";
import type { ShotView } from "../../types/shot";
import { deriveShotStatus } from "./shotDomain";
import { buildShotListView, defaultShotListControls, isShotListFiltered, isShotListReorderDisabled, updateShotListControls } from "./shotListQuery";

const shot = (id: string, ordinal: number, overrides: Partial<ShotView> = {}): ShotView => ({
  id,
  projectId: "project-1",
  ordinal,
  name: `镜头 ${id}`,
  promptText: `Prompt ${id}`,
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

function configuredShot(id: string, ordinal: number, taskStatus?: string): ShotView {
  return shot(id, ordinal, {
    stageConfigs: [{ stage: "image", workflowVersionId: "workflow", recipeId: "recipe", scalarValues: {}, updatedAt: "2026-01-01T00:00:00Z" }],
    generationLinks: taskStatus ? [{ id: `${id}-link`, stage: "image", createdAt: "2026-01-01T00:00:00Z", task: { id: `${id}-task`, status: taskStatus, outputAssetIds: [] } as never }] : [],
  });
}

describe("ShotWorkspace list controls", () => {
  it("searches name and prompt case-insensitively before ordinal sorting", () => {
    const shots = [
      shot("two", 2, { name: "Wide Closeup", promptText: "blue room" }),
      shot("one", 1, { name: "Establishing", promptText: "A RED DOOR" }),
    ];
    expect(buildShotListView(shots, { ...defaultShotListControls(), query: "red door" }).pageShots.map((item) => item.id)).toEqual(["one"]);
    expect(buildShotListView(shots, { ...defaultShotListControls(), query: "WIDE" }).pageShots.map((item) => item.id)).toEqual(["two"]);
  });

  it("filters through the existing derived shot status", () => {
    const shots = [shot("draft", 0), configuredShot("ready", 1), configuredShot("running", 2, "RUNNING")];
    expect(deriveShotStatus(shots[2])).toBe("GENERATING_IMAGE");
    expect(buildShotListView(shots, { ...defaultShotListControls(), status: "GENERATING_IMAGE" }).pageShots.map((item) => item.id)).toEqual(["running"]);
  });

  it("applies display pagination after filtering and reports counts", () => {
    const shots = Array.from({ length: 120 }, (_, index) => shot(String(index + 1), index));
    const result = buildShotListView(shots, { ...defaultShotListControls(), page: 3 });

    expect(result.pageShots).toHaveLength(20);
    expect(result.pageStart).toBe(101);
    expect(result.pageEnd).toBe(120);
    expect(result.filteredCount).toBe(120);
    expect(result.pageCount).toBe(3);
  });

  it("resets to page one when query, status, or page size changes", () => {
    const controls = { ...defaultShotListControls(), page: 4 };
    expect(updateShotListControls(controls, { query: "120" }).page).toBe(1);
    expect(updateShotListControls(controls, { status: "FAILED" }).page).toBe(1);
    expect(updateShotListControls(controls, { pageSize: 25 }).page).toBe(1);
  });

  it("disables reorder only for search/status mode so page size alone keeps reorder semantics", () => {
    expect(isShotListFiltered(defaultShotListControls())).toBe(false);
    expect(isShotListFiltered({ ...defaultShotListControls(), pageSize: 25 })).toBe(false);
    expect(isShotListFiltered({ ...defaultShotListControls(), query: "shot" })).toBe(true);
    expect(isShotListFiltered({ ...defaultShotListControls(), status: "READY" })).toBe(true);
    expect(isShotListReorderDisabled({ ...defaultShotListControls(), query: "shot" })).toBe(true);
    expect(isShotListReorderDisabled(defaultShotListControls())).toBe(false);
  });
});
