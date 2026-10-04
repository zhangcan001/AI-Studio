import type { ProjectOverview } from "../../product/types";

/** Presentation of cached facts only; never an admission or project priority. */
export function runtimeReadinessPresentation(facts: ProjectOverview["runtimeReadiness"]) {
  const connectionLabel = facts.connection === null ? "未检查" : facts.connection === "CONNECTED" ? "已连接"
    : facts.connection === "OFFLINE" ? "离线" : facts.connection === "INCOMPATIBLE" ? "版本不兼容" : "未知";
  const known = facts.status === "READY" || facts.status === "WARNING" || facts.status === "BLOCKED";
  const preflightLabel = facts.status === null ? "尚未预检" : facts.status === "READY" ? "已通过"
    : facts.status === "WARNING" ? "有警告" : facts.status === "BLOCKED" ? "已阻断" : "未知";
  const busy = facts.runtimeBusy || facts.productionBusy || facts.activeTaskCount > 0;
  return {
    connectionLabel,
    preflightLabel,
    workflowsLabel: known ? `${facts.workflowReady} / ${facts.workflowTotal} 可用` : "未检查",
    resourcesLabel: known ? busy ? `忙碌${facts.activeTaskCount > 0 ? ` · 活动任务 ${facts.activeTaskCount}` : ""}` : "空闲" : "未检查",
    needsAttention: facts.connection !== "CONNECTED" || facts.status !== "READY",
  };
}
