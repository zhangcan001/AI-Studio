import type { WorkspaceResume } from "../../types/workspaceResume";
import { isWorkspace } from "../../types/workspaceResume";
import { fromLegacyLocation } from "./legacyAdapter";
import { routeProjectId, type AppRoute } from "./types";
import type { ProjectCommandCenterCollectionFilter } from "../../types/projectCommandCenter";

export const ROUTE_RESUME_KEY = "aistudio.appRoute.v2";
const text = (value: unknown): value is string => typeof value === "string" && value.length > 0 && value.length <= 512;
function parseCollectionFilter(value: unknown): ProjectCommandCenterCollectionFilter | undefined {
  if (!value || typeof value !== "object") return undefined;
  const v = value as Record<string, unknown>;
  if (v.kind === "review" && v.state === "PENDING") return { kind: "review", state: "PENDING" };
  if (v.kind === "tasks" && ["ALL", "ACTIVE", "SUCCEEDED", "FAILED", "CANCELLED"].includes(String(v.status))) return { kind: "tasks", status: v.status as Extract<ProjectCommandCenterCollectionFilter, { kind: "tasks" }>["status"] };
  if (v.kind === "shots" && (v.status === undefined || ["ALL", "DRAFT", "READY", "GENERATING_IMAGE", "IMAGE_REVIEW", "IMAGE_SELECTED", "GENERATING_VIDEO", "VIDEO_REVIEW", "COMPLETED", "FAILED"].includes(String(v.status))) && (v.stage === undefined || v.stage === "image" || v.stage === "video")) return { kind: "shots", status: v.status as Extract<ProjectCommandCenterCollectionFilter, { kind: "shots" }>["status"], stage: v.stage as "image" | "video" | undefined, sceneId: text(v.sceneId) ? v.sceneId : undefined };
  return undefined;
}
/** Accept only a bounded, known locator shape; never trust persisted JSON casts. */
export function parseRoute(value: unknown, depth = 0): AppRoute | undefined {
  if (!value || typeof value !== "object" || depth > 3) return undefined;
  const v = value as Record<string, unknown>;
  if (v.kind === "project-list") return { kind: "project-list" };
  if (v.kind === "system-settings" && text(v.section)) {
    const returnTo = v.returnTo === undefined ? undefined : parseRoute(v.returnTo, depth + 1);
    if (v.returnTo !== undefined && !returnTo) return undefined;
    return { kind: "system-settings", section: v.section, returnTo };
  }
  if (!text(v.projectId)) return undefined;
  const projectId = v.projectId;
  switch (v.kind) {
    case "project": return v.page === "overview" ? { kind: "project", projectId, page: "overview" } : undefined;
    case "create": return (v.stage === "image" || v.stage === "video") && (v.shotId === undefined || text(v.shotId)) && (v.surface === undefined || v.surface === "batch") ? { kind: "create", projectId, stage: v.stage, shotId: v.shotId as string | undefined, surface: v.surface as "batch" | undefined } : undefined;
    case "project-settings": return text(v.section) ? { kind: "project-settings", projectId, section: v.section } : undefined;
    case "runs": {
      const r = v.run as Record<string, unknown> | undefined;
      if (r !== undefined && (!r || !["production-run", "queue-batch", "task"].includes(String(r.source)) || !text(r.id))) return undefined;
      if (v.filter !== undefined && !["all", "completed", "review", "production", "failed", "active", "tasks"].includes(String(v.filter))) return { kind: "runs", projectId };
      const c = v.context && typeof v.context === "object" ? v.context as Record<string, unknown> : {};
      const context = { shotId: text(c.shotId) ? c.shotId : undefined, itemId: text(c.itemId) ? c.itemId : undefined, reviewId: text(c.reviewId) ? c.reviewId : undefined, stage: c.stage === "IMAGE" || c.stage === "VIDEO" ? c.stage : undefined, collectionFilter: parseCollectionFilter(c.collectionFilter) };
      return { kind: "runs", projectId, run: r ? { source: r.source as "production-run" | "queue-batch" | "task", id: r.id as string } : undefined, filter: v.filter as string | undefined, context: Object.values(context).some(Boolean) ? context : undefined };
    }
    case "library": {
      const r = v.resource as Record<string, unknown> | undefined;
      if (r !== undefined && (!r || !["asset", "prompt", "profile", "reference-set"].includes(String(r.kind)) || !text(r.id))) return undefined;
      return { kind: "library", projectId, resource: r ? { kind: r.kind as "asset" | "prompt" | "profile" | "reference-set", id: r.id as string } : undefined, filter: ["all","media","images","videos","audio","prompts","profiles","reference-sets","advanced-assets","advanced-prompts"].includes(String(v.filter)) ? String(v.filter) : undefined };
    }
    default: return undefined;
  }
}
export function resolveResume(payload: unknown, legacy: WorkspaceResume, projectIds: readonly string[]): AppRoute {
  let parsed: AppRoute | undefined;
  if (payload && typeof payload === "object" && (payload as { version?: unknown }).version === 2) {
    const raw = (payload as { route?: unknown }).route;
    parsed = parseRoute(raw);
    // A damaged child locator must not resurrect an unrelated legacy location.
    if (!parsed && raw && typeof raw === "object") {
      const value = raw as Record<string, unknown>;
      if (text(value.projectId)) parsed = value.kind === "runs" ? { kind: "runs", projectId: value.projectId } : value.kind === "library" ? { kind: "library", projectId: value.projectId } : value.kind === "create" ? { kind: "create", projectId: value.projectId, stage: value.stage === "video" ? "video" : "image" } : { kind: "project", projectId: value.projectId, page: "overview" };
    }
  }
  const route = parsed ?? fromLegacyLocation({ projectId: legacy.lastProjectId ?? undefined, workspace: isWorkspace(legacy.lastWorkspace) ? legacy.lastWorkspace : "command-center", shotId: legacy.lastShotId ?? undefined });
  const projectId = routeProjectId(route);
  return projectId && !projectIds.includes(projectId) ? { kind: "project-list" } : route;
}
/** Read-only existence ports. Transport and error presentation remain in the host. */
export interface ResumeReaders {
  shotIds: (projectId: string) => Promise<readonly string[]>;
  runExists: (projectId: string, run: NonNullable<Extract<AppRoute, { kind: "runs" }>["run"]>) => Promise<void>;
  resourceExists?: (projectId: string, resource: NonNullable<Extract<AppRoute,{kind:"library"}>["resource"]>) => Promise<void>;
  assetExists: (projectId: string, assetId: string) => Promise<void>;
}
export async function validateResumeChildren(route: AppRoute, readers: ResumeReaders): Promise<AppRoute> {
  if (route.kind === "system-settings" && route.returnTo) return { ...route, returnTo: await validateResumeChildren(route.returnTo, readers) };
  if (route.kind === "create" && route.shotId) return withoutMissingShot(route, await readers.shotIds(route.projectId));
  try {
    if (route.kind === "runs" && route.run) await readers.runExists(route.projectId, route.run);
    if (route.kind === "library" && route.resource) {
      if(readers.resourceExists) await readers.resourceExists(route.projectId,route.resource);
      else if(route.resource.kind === "asset") await readers.assetExists(route.projectId,route.resource.id);
    }
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
    // An unavailable transport is not evidence that a child was deleted.
    if (code !== "LIBRARY_RESOURCE_NOT_FOUND" && code !== "RUN_NOT_FOUND" && code !== "ASSET_NOT_FOUND" && code !== "PROJECT_SCOPE_VIOLATION") throw error;
    if (route.kind === "runs") return { kind: "runs", projectId: route.projectId, filter: route.filter };
    if (route.kind === "library") return { kind: "library", projectId: route.projectId, filter: route.filter };
  }
  return route;
}
export function withoutMissingShot(route: AppRoute, shotIds: readonly string[]): AppRoute {
  if (route.kind === "system-settings" && route.returnTo) return { ...route, returnTo: withoutMissingShot(route.returnTo, shotIds) };
  return route.kind === "create" && route.shotId && !shotIds.includes(route.shotId) ? { ...route, shotId: undefined } : route;
}
export function readRouteResume(): unknown {
  try { const raw = localStorage.getItem(ROUTE_RESUME_KEY); return raw && raw.length <= 8192 ? JSON.parse(raw) : undefined; } catch { return undefined; }
}
export function writeRouteResume(route: AppRoute): void {
  try { localStorage.setItem(ROUTE_RESUME_KEY, JSON.stringify({ version: 2, route })); } catch { /* Navigation must survive unavailable convenience storage. */ }
}
