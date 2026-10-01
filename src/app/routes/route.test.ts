import { describe, expect, it } from "vitest";
import { appRouteReducer, initialRouteState } from "./reducer";
import { fromLegacyLocation, toLegacyLocation } from "./legacyAdapter";
import type { AppRoute } from "./types";

describe("canonical route transitions", () => {
  it("navigates overview, create stages, runs, library and project settings", () => {
    const routes: AppRoute[] = [
      { kind: "project", projectId: "A", page: "overview" },
      { kind: "create", projectId: "A", stage: "image", shotId: "shotA" },
      { kind: "create", projectId: "A", stage: "video", shotId: "shotA" },
      { kind: "runs", projectId: "A" },
      { kind: "runs", projectId: "A", run: { source: "queue-batch", id: "batchA" } },
      { kind: "library", projectId: "A", resource: { kind: "asset", id: "assetA" } },
      { kind: "library", projectId: "A" },
      { kind: "project-settings", projectId: "A", section: "generators" },
    ];
    let state = initialRouteState;
    for (const route of routes) { state = appRouteReducer(state, { type: "navigate", route }); expect(state.current).toEqual(route); }
    expect(state.history.length).toBeLessThanOrEqual(20);
    expect(appRouteReducer(state, { type: "back" }).current).toEqual({ kind: "project", projectId: "A", page: "overview" });
  });
  it("returns system settings to the exact originating route", () => {
    const create: AppRoute = { kind: "create", projectId: "A", shotId: "shotA", stage: "video" };
    let state = appRouteReducer(initialRouteState, { type: "navigate", route: create });
    state = appRouteReducer(state, { type: "navigate", route: { kind: "system-settings", section: "general" } });
    expect(appRouteReducer(state, { type: "back" }).current).toEqual(create);
  });
  it("backs details to the nearest collection, including direct-entry locators", () => {
    for (const route of [
      { kind: "create", projectId: "A", shotId: "shot", stage: "image" },
      { kind: "runs", projectId: "A", run: { source: "task", id: "task" } },
      { kind: "library", projectId: "A", resource: { kind: "asset", id: "asset" } },
    ] satisfies AppRoute[]) {
      const state = appRouteReducer(initialRouteState, { type: "restore", route });
      const parent = appRouteReducer(state, { type: "back" }).current;
      expect(parent.kind).toBe(route.kind);
      expect(JSON.stringify(parent)).not.toMatch(/shot|task|asset/);
    }
  });
  it("switches A deep route to B overview, clears A locators, restores A", () => {
    for (const route of [
      { kind: "create", projectId: "A", shotId: "shotA", stage: "image" },
      { kind: "runs", projectId: "A", run: { source: "task", id: "taskA" } },
      { kind: "library", projectId: "A", resource: { kind: "asset", id: "assetA" } },
    ] satisfies AppRoute[]) {
      let state = appRouteReducer(initialRouteState, { type: "navigate", route });
      state = appRouteReducer(state, { type: "switch-project", projectId: "B" });
      expect(state.current).toEqual({ kind: "project", projectId: "B", page: "overview" });
      expect(state.history).toEqual([]);
      state = appRouteReducer(state, { type: "switch-project", projectId: "A" });
      expect(state.current).toEqual(route);
    }
  });
});

describe("legacy matrix and one-way projection", () => {
  it("maps every retained workspace and shot section", () => {
    const matrix = [
      ["command-center", "project", "project"], ["shots", "creation", "create"],
      ["shots", "production", "runs"], ["shots", "review", "runs"],
      ["studio", "creation", "create"], ["video", "production", "create"],
      ["assets", "assets", "library"], ["prompts", "prompts", "library"],
      ["tasks", "review", "runs"], ["projects", "project", "project-settings"],
      ["workflows", "workflows", "project-settings"], ["settings", "settings", "system-settings"],
      ["tools", "tools", "system-settings"],
    ] as const;
    for (const [workspace, section, kind] of matrix) {
      const route = fromLegacyLocation({ projectId: "A", workspace, section });
      expect(route.kind).toBe(kind);
      expect(toLegacyLocation(route).workspace).toBe(workspace);
    }
  });
  it("preserves review item context and exact task/asset locators", () => {
    const review = fromLegacyLocation({ projectId: "A", workspace: "shots", section: "review", reviewId: "reviewA", batchId: "batchA", shotId: "shotA" });
    expect(toLegacyLocation(review)).toMatchObject({ reviewId: "reviewA", batchId: "batchA", shotId: "shotA" });
    expect(toLegacyLocation(fromLegacyLocation({ projectId: "A", workspace: "tasks", taskId: "taskA" })).taskId).toBe("taskA");
    expect(toLegacyLocation(fromLegacyLocation({ projectId: "A", workspace: "assets", assetId: "assetA" })).assetId).toBe("assetA");
  });
});
