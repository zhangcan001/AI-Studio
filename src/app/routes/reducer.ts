import { routeProjectId, type AppRoute } from "./types";

export interface RouteState { current: AppRoute; history: AppRoute[]; projectLocations: Record<string, AppRoute> }
export type RouteAction = { type: "navigate"; route: AppRoute } | { type: "back" } | { type: "switch-project"; projectId: string } | { type: "restore"; route: AppRoute };
export const initialRouteState: RouteState = { current: { kind: "project-list" }, history: [], projectLocations: {} };

function parent(route: AppRoute): AppRoute {
  switch (route.kind) {
    case "system-settings": return route.returnTo ?? { kind: "project-list" };
    case "runs": return route.run ? { kind: "runs", projectId: route.projectId } : { kind: "project", projectId: route.projectId, page: "overview" };
    case "library": return route.resource ? { kind: "library", projectId: route.projectId, filter: route.filter } : { kind: "project", projectId: route.projectId, page: "overview" };
    case "create": return route.shotId ? { kind: "create", projectId: route.projectId, stage: route.stage } : { kind: "project", projectId: route.projectId, page: "overview" };
    case "project-settings": return { kind: "project", projectId: route.projectId, page: "overview" };
    default: return { kind: "project-list" };
  }
}
export function appRouteReducer(state: RouteState, action: RouteAction): RouteState {
  const projectId = routeProjectId(state.current);
  const locations = projectId && state.current.kind !== "system-settings" ? { ...state.projectLocations, [projectId]: state.current } : state.projectLocations;
  if (action.type === "restore") return { ...state, current: action.route, history: [], projectLocations: locations };
  if (action.type === "switch-project") return { current: locations[action.projectId] ?? { kind: "project", projectId: action.projectId, page: "overview" }, history: [], projectLocations: locations };
  if (action.type === "back") {
    // Advanced Library is an explicit editor bridge, not a resource detail parent.
    // Return to the canonical Library origin; a resumed editor has no history.
    if (state.current.kind === "library" && ["advanced-assets", "advanced-prompts"].includes(state.current.filter ?? "")) {
      const origin = state.history[state.history.length - 1];
      const current: AppRoute = origin?.kind === "library" && origin.projectId === state.current.projectId && !["advanced-assets", "advanced-prompts"].includes(origin.filter ?? "")
        ? origin : { ...state.current, filter: state.current.filter === "advanced-prompts" ? "prompts" : "all" };
      return { current, history: state.history.slice(0, -1), projectLocations: locations };
    }
    // Details always return to their collection, even when entered by a deep link.
    const detail = (state.current.kind === "runs" && state.current.run) || (state.current.kind === "library" && state.current.resource) || (state.current.kind === "create" && state.current.shotId) || state.current.kind === "system-settings" || state.current.kind === "project-settings";
    return { current: detail ? parent(state.current) : state.history[state.history.length - 1] ?? parent(state.current), history: state.history.slice(0, -1), projectLocations: locations };
  }
  const route = action.route.kind === "system-settings" && !action.route.returnTo ? { ...action.route, returnTo: state.current } : action.route;
  if (JSON.stringify(route) === JSON.stringify(state.current)) return state;
  // Do not retain another project's child locators in the Back stack.
  const sameProject = routeProjectId(route) === projectId;
  return { current: route, history: sameProject ? [...state.history, state.current].slice(-20) : [], projectLocations: locations };
}
