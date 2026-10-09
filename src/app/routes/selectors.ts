import type { AppRoute } from "./types";
export function routeTitle(route: AppRoute): string {
  switch (route.kind) {
    case "project-list": return "项目";
    case "project": return "概览";
    case "create": return route.stage === "video" ? "视频创作" : "历史图片";
    case "runs": return "运行";
    case "library": return route.filter === "prompts" ? "提示词" : "素材库";
    case "project-settings": return "项目设置";
    case "system-settings": return "系统设置";
  }
}
