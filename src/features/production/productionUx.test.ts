import { describe, expect, it } from "vitest";
import { productionQueueAction, recentProductionQueues, summarizeProductionQueues, type ProductionUxQueueRecord } from "./productionUx";

const queues: ProductionUxQueueRecord[] = [
  { id: "q-old", name: "旧队列", status: "SUCCEEDED", total: 2, succeeded: 2, failed: 0, active: 0, updatedAt: "2026-08-09T00:00:00Z" },
  { id: "q-new", name: "新队列", status: "RUNNING", total: 3, succeeded: 1, failed: 1, active: 1, updatedAt: "2026-08-10T00:00:00Z" },
];

describe("Pack 08 production UX source contracts", () => {
  it("summarizes queue health and recency without changing queue semantics", () => {
    expect(summarizeProductionQueues(queues)).toEqual({
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
    expect(productionQueueAction("DRAFT")).toBe("开始");
    expect(productionQueueAction("PAUSED")).toBe("继续");
    expect(productionQueueAction("RUNNING")).toBe("暂停");
    expect(productionQueueAction("SUCCEEDED")).toBe("查看");
  });
});
