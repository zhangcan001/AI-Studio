export type ProductionUxQueueStatus = "DRAFT" | "RUNNING" | "PAUSED" | "SUCCEEDED" | "FAILED" | "CANCELLED" | "ARCHIVED";

export interface ProductionUxQueueRecord {
  id: string;
  name: string;
  status: ProductionUxQueueStatus;
  total: number;
  succeeded: number;
  failed: number;
  active: number;
  updatedAt: string;
}

export interface ProductionUxDashboardSummary {
  queueCount: number;
  runningCount: number;
  activeItemCount: number;
  succeededItemCount: number;
  failedItemCount: number;
  latestQueueId?: string;
}

export function summarizeProductionQueues(
  queues: readonly ProductionUxQueueRecord[],
): ProductionUxDashboardSummary {
  const recent = [...queues].sort((left, right) => {
    const time = right.updatedAt.localeCompare(left.updatedAt);
    return time || right.id.localeCompare(left.id);
  });
  return {
    queueCount: queues.length,
    runningCount: queues.filter((queue) => queue.status === "RUNNING").length,
    activeItemCount: queues.reduce((total, queue) => total + queue.active, 0),
    succeededItemCount: queues.reduce((total, queue) => total + queue.succeeded, 0),
    failedItemCount: queues.reduce((total, queue) => total + queue.failed, 0),
    latestQueueId: recent[0]?.id,
  };
}

export function recentProductionQueues(
  queues: readonly ProductionUxQueueRecord[],
  limit = 5,
): ProductionUxQueueRecord[] {
  if (!Number.isInteger(limit) || limit < 1) return [];
  return [...queues]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || right.id.localeCompare(left.id))
    .slice(0, limit);
}

export function productionQueueAction(status: ProductionUxQueueStatus): "开始" | "继续" | "暂停" | "查看" {
  switch (status) {
    case "DRAFT":
      return "开始";
    case "PAUSED":
      return "继续";
    case "RUNNING":
      return "暂停";
    default:
      return "查看";
  }
}
