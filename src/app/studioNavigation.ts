import type { StudioRailItemId } from "../components/studio/StudioGlobalRail";
import type { Workspace } from "../types/workspaceResume";

export type StudioSection = StudioRailItemId;
export type ShotWorkspaceMode = "creation" | "production" | "review";

export interface StudioRoute {
  workspace: Workspace;
  section: StudioSection;
}

const STUDIO_SECTION_ROUTES: Record<StudioSection, StudioRoute> = {
  project: { workspace: "command-center", section: "project" },
  creation: { workspace: "shots", section: "creation" },
  assets: { workspace: "assets", section: "assets" },
  production: { workspace: "shots", section: "production" },
  review: { workspace: "shots", section: "review" },
  analysis: { workspace: "command-center", section: "analysis" },
  settings: { workspace: "settings", section: "settings" },
};

export function studioRouteForSection(section: StudioSection): StudioRoute {
  return STUDIO_SECTION_ROUTES[section];
}

export function defaultStudioSectionForWorkspace(workspace: Workspace): StudioSection {
  switch (workspace) {
    case "command-center":
    case "projects":
      return "project";
    case "assets":
      return "assets";
    case "shots":
    case "studio":
    case "workflows":
      return "creation";
    case "video":
    case "tasks":
      return workspace === "tasks" ? "review" : "production";
    case "settings":
      return "settings";
  }
}

export function shotWorkspaceModeForSection(section: StudioSection): ShotWorkspaceMode {
  if (section === "production") return "production";
  if (section === "review") return "review";
  return "creation";
}
