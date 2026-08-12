import { beforeEach, describe, expect, it } from "vitest";
import type { ProductionBatchSummary } from "../../types/productionQueue";
import {
  readStoredProductionQueueId,
  rememberProductionQueue,
  selectProductionQueueId,
} from "./productionQueueSelection";

const queues: ProductionBatchSummary[] = [
  {
    id: "batch-latest",
    projectId: "project-1",
    name: "Latest",
    status: "COMPLETED",
    continueOnFailure: true,
    createdAt: "2026-08-12T00:00:00Z",
    updatedAt: "2026-08-12T00:00:00Z",
  },
  {
    id: "batch-running",
    projectId: "project-1",
    name: "Running",
    status: "RUNNING",
    continueOnFailure: true,
    createdAt: "2026-08-11T00:00:00Z",
    updatedAt: "2026-08-11T00:00:00Z",
  },
];

describe("production queue selection persistence", () => {
  beforeEach(() => {
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value),
        removeItem: (key: string) => values.delete(key),
      },
    });
  });

  it("restores the last opened queue before choosing a newer queue", () => {
    rememberProductionQueue("project-1", "batch-running");
    expect(readStoredProductionQueueId("project-1")).toBe("batch-running");
    expect(selectProductionQueueId(queues, [readStoredProductionQueueId("project-1")], true)).toBe("batch-running");
  });

  it("falls back to an active queue when the saved queue no longer exists", () => {
    expect(selectProductionQueueId(queues, ["deleted-batch"], true)).toBe("batch-running");
  });

  it("does not invent a selection for the full queue list", () => {
    expect(selectProductionQueueId(queues, [], false)).toBeUndefined();
  });
});
