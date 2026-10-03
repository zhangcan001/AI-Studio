import type { RunRef } from "../../product/types";
import type { ProjectCommandCenterCollectionFilter } from "../../types/projectCommandCenter";

export type { ResourceRef } from "../../product/libraryTypes";
import type { ResourceRef } from "../../product/libraryTypes";
/** Historical intent carried by canonical Runs; never an alternate UI owner. */
export interface LegacyRunContext {
  taskId?: string;
  batchId?: string;
  assetId?: string;
  shotId?: string;
  itemId?: string;
  reviewId?: string;
  stage?: string;
  collectionFilter?: ProjectCommandCenterCollectionFilter;
}
export type AppRoute =
  | { kind: "project-list" }
  | { kind: "project"; projectId: string; page: "overview" }
  | { kind: "create"; projectId: string; shotId?: string; stage: "image" | "video"; surface?: "batch" }
  | { kind: "runs"; projectId: string; run?: RunRef; filter?: string; context?: LegacyRunContext }
  | { kind: "library"; projectId: string; resource?: ResourceRef; filter?: string }
  | { kind: "project-settings"; projectId: string; section: string }
  | { kind: "system-settings"; section: string; returnTo?: AppRoute };

export function routeProjectId(route: AppRoute): string | undefined {
  return "projectId" in route ? route.projectId : route.kind === "system-settings" && route.returnTo ? routeProjectId(route.returnTo) : undefined;
}
