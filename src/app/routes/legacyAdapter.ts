import type { Workspace } from "../../types/workspaceResume";
import type { StudioSection } from "../studioNavigation";
import { defaultStudioSectionForWorkspace } from "../studioNavigation";
import type { AppRoute, LegacyRunContext } from "./types";

export interface LegacyLocation extends LegacyRunContext {
  projectId?: string; workspace: Workspace; section?: StudioSection;
  taskId?: string; batchId?: string; assetId?: string;
}
/** Only for startup/imported targets and explicit legacy navigation events. */
export function fromLegacyLocation(location: LegacyLocation): AppRoute {
  const { projectId, workspace, shotId } = location;
  const section = location.section ?? defaultStudioSectionForWorkspace(workspace);
  if (workspace === "settings" || workspace === "tools") return { kind: "system-settings", section: workspace === "tools" ? "advanced-tools" : "general", returnTo: projectId ? { kind: "project", projectId, page: "overview" } : undefined };
  if (!projectId) return workspace === "workflows" ? { kind: "system-settings", section: "advanced-workflows" } : { kind: "project-list" };
  switch (workspace) {
    case "projects": return { kind: "project-settings", projectId, section: "general" };
    case "workflows": return { kind: "project-settings", projectId, section: "advanced-workflows" };
    case "assets": return { kind: "library", projectId, resource: location.assetId ? { kind: "asset", id: location.assetId } : undefined };
    case "prompts": return { kind: "library", projectId, filter: "prompts" };
    case "tasks":
    case "shots": {
      if (workspace === "shots" && section !== "production" && section !== "review") return { kind: "create", projectId, shotId, stage: (location.stage === "VIDEO" || location.stage === "video") ? "video" : "image" };
      const context = { shotId, itemId: location.itemId, reviewId: location.reviewId, stage: location.stage, collectionFilter: location.collectionFilter };
      return { kind: "runs", projectId, run: location.taskId && !location.reviewId ? { source: "task", id: location.taskId } : location.batchId ? { source: "queue-batch", id: location.batchId } : undefined, filter: workspace === "tasks" ? "tasks" : section === "review" ? "review" : "production", context };
    }
    case "studio": return { kind: "create", projectId, stage: "image", surface: "batch" };
    case "video": return { kind: "create", projectId, stage: "video", surface: "batch" };
    default: return { kind: "project", projectId, page: "overview" };
  }
}
/** Pure, one-way projection. No store reads, persistence, or domain mutation. */
export function toLegacyLocation(route: AppRoute): LegacyLocation {
  switch (route.kind) {
    case "project-list": return { workspace: "projects", section: "project" };
    case "project": return { projectId: route.projectId, workspace: "command-center", section: "project" };
    case "create": return { projectId: route.projectId, workspace: route.surface === "batch" ? route.stage === "video" ? "video" : "studio" : "shots", section: "creation", shotId: route.shotId, stage: route.stage.toUpperCase(), collectionFilter: { kind: "shots", status: "ALL", stage: route.stage } };
    case "runs": return { ...route.context, projectId: route.projectId, workspace: route.run?.source === "task" || route.filter === "tasks" ? "tasks" : "shots", section: route.filter === "review" || route.filter === "tasks" ? "review" : "production", taskId: route.run?.source === "task" ? route.run.id : undefined, batchId: route.run?.source === "queue-batch" ? route.run.id : undefined };
    case "library": return { projectId: route.projectId, workspace: route.filter === "prompts" || route.resource?.kind === "prompt" ? "prompts" : "assets", section: route.filter === "prompts" ? "prompts" : "assets", assetId: route.resource?.kind === "asset" ? route.resource.id : undefined };
    case "project-settings": return { projectId: route.projectId, workspace: route.section === "generators" || route.section === "advanced-workflows" ? "workflows" : "projects", section: route.section === "generators" || route.section === "advanced-workflows" ? "workflows" : "project" };
    case "system-settings": return { workspace: route.section === "advanced-tools" ? "tools" : route.section === "advanced-workflows" ? "workflows" : "settings", section: route.section === "advanced-tools" ? "tools" : route.section === "advanced-workflows" ? "workflows" : "settings" };
  }
}
