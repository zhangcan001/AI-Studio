import type { ProjectCommandCenterNavigationRequest } from "../../features/projects/ProjectCommandCenter";
import type { Workspace } from "../../types/workspaceResume";
import type { ProjectCommandCenterCollectionFilter } from "../../types/projectCommandCenter";
import { defaultStudioSectionForWorkspace, studioRouteForSection, type StudioSection } from "../studioNavigation";

export interface ResolvedProjectCommandCenterNavigation {
  projectId?: string;
  workspace: Workspace;
  section: StudioSection;
  shotId?: string;
  batchId?: string;
  itemId?: string;
  reviewId?: string;
  taskId?: string;
  assetId?: string;
  stage?: string;
  collectionFilter?: ProjectCommandCenterCollectionFilter;
}

export function resolveProjectCommandCenterNavigation(
  request: ProjectCommandCenterNavigationRequest,
): ResolvedProjectCommandCenterNavigation {
  const reviewId = request.reviewId ?? request.itemId;
  const target = {
    ...(request.projectId ? { projectId: request.projectId } : {}),
    shotId: request.shotId,
    batchId: request.batchId,
    ...(request.itemId ? { itemId: request.itemId } : {}),
    ...(reviewId ? { reviewId } : {}),
    ...(request.taskId ? { taskId: request.taskId } : {}),
    ...(request.assetId ? { assetId: request.assetId } : {}),
    ...(request.stage ? { stage: request.stage } : {}),
    ...(request.collectionFilter ? { collectionFilter: request.collectionFilter } : {}),
  };

  // A review item is the primary target when present; task/batch/shot/asset
  // IDs remain context and must not replace the review authority.
  if (reviewId) return { ...target, workspace: "shots", section: "review" };
  if (request.taskId) return { ...target, workspace: "tasks", section: "review" };
  if (request.batchId) return { ...target, workspace: "shots", section: "production" };
  if (request.assetId) return { ...target, workspace: "assets", section: "assets" };
  if (request.shotId) {
    const section = request.section === "production" ? "production" : "creation";
    return { ...target, workspace: "shots", section };
  }
  if (request.collectionFilter?.kind === "tasks") return { ...target, workspace: "tasks", section: "review" };
  if (request.collectionFilter?.kind === "review") return { ...target, workspace: "shots", section: "review" };
  if (request.collectionFilter?.kind === "shots") {
    const section = request.section === "production" ? "production" : "creation";
    return { ...target, workspace: "shots", section };
  }
  if (request.section) {
    const route = studioRouteForSection(request.section);
    return { ...target, workspace: route.workspace, section: route.section };
  }
  if (request.destination === "studio" || request.destination === "shots") {
    return { ...target, workspace: "shots", section: "creation" };
  }
  return {
    ...target,
    workspace: request.destination,
    section: defaultStudioSectionForWorkspace(request.destination),
  };
}
