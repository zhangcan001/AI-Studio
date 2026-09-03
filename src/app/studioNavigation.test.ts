import { describe, expect, it } from "vitest";
import {
  defaultStudioSectionForWorkspace,
  shotWorkspaceModeForSection,
  studioRouteForSection,
} from "./studioNavigation";

describe("studio section routes", () => {
  it("keeps global navigation semantic instead of overloading the old studio workspace", () => {
    expect(studioRouteForSection("project")).toEqual({ workspace: "command-center", section: "project" });
    expect(studioRouteForSection("creation")).toEqual({ workspace: "shots", section: "creation" });
    expect(studioRouteForSection("assets")).toEqual({ workspace: "assets", section: "assets" });
    expect(studioRouteForSection("production")).toEqual({ workspace: "shots", section: "production" });
    expect(studioRouteForSection("review")).toEqual({ workspace: "shots", section: "review" });
    expect(studioRouteForSection("workflows")).toEqual({ workspace: "workflows", section: "workflows" });
    expect(studioRouteForSection("analysis")).toEqual({ workspace: "command-center", section: "analysis" });
    expect(studioRouteForSection("settings")).toEqual({ workspace: "settings", section: "settings" });
  });

  it("derives a stable mode for resumed legacy workspaces", () => {
    expect(defaultStudioSectionForWorkspace("shots")).toBe("creation");
    expect(defaultStudioSectionForWorkspace("video")).toBe("production");
    expect(defaultStudioSectionForWorkspace("tasks")).toBe("review");
    expect(defaultStudioSectionForWorkspace("workflows")).toBe("workflows");
    expect(shotWorkspaceModeForSection("creation")).toBe("creation");
    expect(shotWorkspaceModeForSection("production")).toBe("production");
    expect(shotWorkspaceModeForSection("review")).toBe("review");
  });
});
