import { describe, expect, it } from "vitest";
import { productionQueueAction, recentProductionQueues, recentPromptEntries, recentWorkflowRecords, summarizeProductionQueues, type ProductionUxQueueRecord } from "./productionUx";

const queues: ProductionUxQueueRecord[] = [
  { id: "q-old", projectId: "project-1", name: "旧队列", status: "COMPLETED", continueOnFailure: true, createdAt: "2026-08-09T00:00:00Z", updatedAt: "2026-08-09T00:00:00Z" },
  { id: "q-new", projectId: "project-1", name: "新队列", status: "RUNNING", continueOnFailure: true, createdAt: "2026-08-10T00:00:00Z", updatedAt: "2026-08-10T00:00:00Z" },
];

describe("Pack 08 production UX source contracts", () => {
  it("summarizes queue health and recency without changing queue semantics", () => {
    expect(summarizeProductionQueues(queues, {
      totalQueues: 2,
      runningQueues: 1,
      pausedQueues: 0,
      completedQueues: 1,
      archivedQueues: 0,
      totalItems: 5,
      pendingItems: 0,
      activeItems: 1,
      succeededItems: 3,
      failedItems: 1,
      cancelledItems: 0,
      skippedItems: 0,
    })).toEqual({
      queueCount: 2,
      runningCount: 1,
      activeItemCount: 1,
      succeededItemCount: 3,
      failedItemCount: 1,
      latestQueueId: "q-new",
    });
    expect(recentProductionQueues(queues, 1).map((queue) => queue.id)).toEqual(["q-new"]);
  });

  it("derives safe queue actions from persisted status", () => {
    expect(productionQueueAction("READY")).toBe("开始");
    expect(productionQueueAction("PAUSED")).toBe("继续");
    expect(productionQueueAction("RUNNING")).toBe("暂停");
    expect(productionQueueAction("COMPLETED")).toBe("查看");
    expect(productionQueueAction("READY", "2026-08-10T00:00:00Z")).toBe("查看");
  });

  it("deduplicates recent workflows and orders recent prompts", () => {
    expect(recentWorkflowRecords([
      { workflowVersionId: "w1", recipeId: "r1", workflowName: "A", lastUsedAt: "2026-08-09T00:00:00Z" },
      { workflowVersionId: "w1", recipeId: "r1", workflowName: "A", lastUsedAt: "2026-08-10T00:00:00Z" },
      { workflowVersionId: "w2", recipeId: "r2", workflowName: "B", lastUsedAt: "2026-08-08T00:00:00Z" },
    ])).toMatchObject([{ workflowVersionId: "w1" }, { workflowVersionId: "w2" }]);
    expect(recentPromptEntries([
      { id: "p1", projectId: "project-1", kind: "prompt", name: "旧", tags: [], createdAt: "2026-08-09T00:00:00Z", updatedAt: "2026-08-09T00:00:00Z", versionCount: 1, versions: [] },
      { id: "p2", projectId: "project-1", kind: "prompt", name: "新", tags: [], createdAt: "2026-08-10T00:00:00Z", updatedAt: "2026-08-10T00:00:00Z", versionCount: 1, versions: [] },
    ])[0].id).toBe("p2");
  });
});
