// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { parseRoute, readRouteResume, resolveResume, ROUTE_RESUME_KEY, validateResumeChildren, withoutMissingShot, writeRouteResume } from "./resumeAdapter";
afterEach(() => localStorage.clear());
describe("versioned route resume", () => {
  it("reads legacy v1 and writes/restarts canonical v2", () => {
    const route = resolveResume(undefined, { lastProjectId: "A", lastWorkspace: "shots", lastShotId: "shotA" }, ["A"]);
    expect(route).toEqual({ kind: "create", projectId: "A", stage: "image", shotId: "shotA" });
    writeRouteResume(route);
    expect(resolveResume(readRouteResume(), {}, ["A"])).toEqual(route);
  });
  it("rejects invalid shape, missing project and future versions safely", () => {
    expect(parseRoute({ kind: "create", projectId: "A", stage: "audio" })).toBeUndefined();
    expect(resolveResume({ version: 2, route: { kind: "project", projectId: "deleted", page: "overview" } }, {}, ["A"])).toEqual({ kind: "project-list" });
    expect(resolveResume({ version: 9, route: {} }, {}, ["A"])).toEqual({ kind: "project-list" });
    localStorage.setItem(ROUTE_RESUME_KEY, "{broken");
    expect(readRouteResume()).toBeUndefined();
    expect(parseRoute({ kind: "runs", projectId: "A", run: { source: "invented", id: "x" } })).toBeUndefined();
  });
  it("falls missing shots/invalid filters back to their own parent", () => {
    expect(withoutMissingShot({ kind: "create", projectId: "A", stage: "video", shotId: "deleted" }, [])).toEqual({ kind: "create", projectId: "A", stage: "video", shotId: undefined });
    expect(parseRoute({ kind: "runs", projectId: "A", filter: "invalid" })).toEqual({ kind: "runs", projectId: "A" });
  });
  it("restarts every parent route and retains review filters", () => {
    for (const route of [{ kind: "project", projectId: "A", page: "overview" }, { kind: "runs", projectId: "A" }, { kind: "library", projectId: "A" }, { kind: "runs", projectId: "A", context: { collectionFilter: { kind: "review", state: "PENDING" } } }] as const) {
      writeRouteResume(route);
      expect(resolveResume(readRouteResume(), {}, ["A"])).toEqual(route);
    }
    expect(resolveResume({ version: 2, route: { kind: "runs", projectId: "A", run: { source: "bad", id: "x" } } }, { lastProjectId: "B", lastWorkspace: "assets" }, ["A", "B"])).toEqual({ kind: "runs", projectId: "A" });
  });
  it("validates nested children without erasing locators on transport failure", async () => {
    const readers = { shotIds: vi.fn().mockResolvedValue(["shotA"]), runExists: vi.fn().mockResolvedValue(undefined), assetExists: vi.fn().mockResolvedValue(undefined) };
    const route = { kind: "system-settings", section: "general", returnTo: { kind: "create", projectId: "A", stage: "image", shotId: "shotA" } } as const;
    expect(await validateResumeChildren(route, readers)).toEqual(route);
    expect(readers.shotIds).toHaveBeenCalledWith("A");
    readers.runExists.mockRejectedValue({ code: "RUN_NOT_FOUND" });
    expect(await validateResumeChildren({ kind: "runs", projectId: "A", run: { source: "task", id: "gone" } }, readers)).toEqual({ kind: "runs", projectId: "A", filter: undefined });
    readers.assetExists.mockRejectedValue({ code: "ASSET_NOT_FOUND" });
    expect(await validateResumeChildren({ kind: "library", projectId: "A", resource: { kind: "asset", id: "gone" } }, readers)).toEqual({ kind: "library", projectId: "A", filter: undefined });
    readers.runExists.mockRejectedValue({ code: "INTERNAL_ERROR" });
    await expect(validateResumeChildren({ kind: "runs", projectId: "A", run: { source: "task", id: "kept" } }, readers)).rejects.toEqual({ code: "INTERNAL_ERROR" });
  });
});
