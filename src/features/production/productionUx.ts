import type { PromptEntryView } from "../../types/prompt";
import type {
  ProductionBatchStatus,
  ProductionBatchSummary,
  ProductionQueueOverview,
} from "../../types/productionQueue";

export type ProductionUxQueueRecord = ProductionBatchSummary;

export interface ProductionUxDashboardSummary {
  queueCount: number;
  runningCount: number;
  activeItemCount: number;
  succeededItemCount: number;
  failedItemCount: number;
  latestQueueId?: string;
}

export interface RecentWorkflowRecord {
  workflowVersionId: string;
  recipeId: string;
  workflowName: string;
  lastUsedAt: string;
}

export function summarizeProductionQueues(
  queues: readonly ProductionBatchSummary[],
  overview?: ProductionQueueOverview,
): ProductionUxDashboardSummary {
  const recent = recentProductionQueues(queues, 1);
  return {
    queueCount: overview?.totalQueues ?? queues.filter((queue) => !queue.archivedAt).length,
    runningCount: overview?.runningQueues ?? queues.filter((queue) => queue.status === "RUNNING").length,
    activeItemCount: overview?.activeItems ?? 0,
    succeededItemCount: overview?.succeededItems ?? 0,
    failedItemCount: overview?.failedItems ?? 0,
    latestQueueId: recent[0]?.id,
  };
}

export function recentProductionQueues(
  queues: readonly ProductionBatchSummary[],
  limit = 5,
): ProductionBatchSummary[] {
  if (!Number.isInteger(limit) || limit < 1) return [];
  return [...queues]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || right.id.localeCompare(left.id))
    .slice(0, limit);
}

export function productionQueueAction(
  status: ProductionBatchStatus,
  archivedAt?: string,
): "开始" | "继续" | "暂停" | "查看" {
  if (archivedAt) return "查看";
  switch (status) {
    case "READY":
      return "开始";
    case "PAUSED":
      return "继续";
    case "RUNNING":
    case "COMPLETED":
      return status === "RUNNING" ? "暂停" : "查看";
  }
}

export function recentWorkflowRecords(
  records: readonly RecentWorkflowRecord[],
  limit = 5,
): RecentWorkflowRecord[] {
  const byPair = new Map<string, RecentWorkflowRecord>();
  [...records]
    .sort((left, right) => right.lastUsedAt.localeCompare(left.lastUsedAt))
    .forEach((record) => {
      const key = `${record.workflowVersionId}:${record.recipeId}`;
      if (!byPair.has(key)) byPair.set(key, record);
    });
  return [...byPair.values()].slice(0, Math.max(0, limit));
}

export function recentPromptEntries(
  entries: readonly PromptEntryView[],
  limit = 5,
): PromptEntryView[] {
  return [...entries]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || right.id.localeCompare(left.id))
    .slice(0, Math.max(0, limit));
}
