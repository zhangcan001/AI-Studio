import { describe, expect, it } from "vitest";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import {
  emptySequentialBatchStartState,
  isCleanSequentialCompletion,
  isStructuredProductionQueueBusy,
  sequentialBatchStartReducer,
} from "./shotQueueState";

function batch(overrides: Partial<ProductionBatchDetail> = {}): ProductionBatchDetail {
  return {
    id: "batch-a",
    projectId: "project-1",
    name: "Batch A",
    status: "COMPLETED",
    continueOnFailure: false,
    createdAt: "2026-09-08T00:00:00Z",
    updatedAt: "2026-09-08T00:00:00Z",
    total: 1,
    pending: 0,
    running: 0,
    succeeded: 1,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [],
    ...overrides,
  };
}

describe("shot sequential queue state", () => {
  it("queues a batch once and preserves the active batch", () => {
    const active = sequentialBatchStartReducer(emptySequentialBatchStartState, { type: "START_ACTIVE", batchId: "batch-a" });
    const queued = sequentialBatchStartReducer(active, { type: "QUEUE_BATCH", batchId: "batch-b" });
    const duplicate = sequentialBatchStartReducer(queued, { type: "QUEUE_BATCH", batchId: "batch-b" });

    expect(queued).toMatchObject({ status: "ACTIVE", currentBatchId: "batch-a", queuedBatchIds: ["batch-b"] });
    expect(duplicate).toBe(queued);
  });

  it("cancels only queued work, or the complete suffix", () => {
    const queued = {
      status: "ACTIVE" as const,
      currentBatchId: "batch-a",
      queuedBatchIds: ["batch-b", "batch-c"],
    };
    expect(sequentialBatchStartReducer(queued, { type: "REMOVE_QUEUED", batchId: "batch-b" }).queuedBatchIds).toEqual(["batch-c"]);
    expect(sequentialBatchStartReducer(queued, { type: "CANCEL_FUTURE" })).toMatchObject({
      status: "ACTIVE",
      currentBatchId: "batch-a",
      queuedBatchIds: [],
    });
  });

  it("pauses and resumes without creating a second state source", () => {
    const paused = sequentialBatchStartReducer(
      { status: "ACTIVE", currentBatchId: "batch-a", queuedBatchIds: ["batch-b"] },
      { type: "PAUSE", reason: "上一批存在失败、取消或跳过项。", canResume: true },
    );
    const resumed = sequentialBatchStartReducer(paused, { type: "RESUME" });

    expect(paused).toMatchObject({ status: "PAUSED", canResume: true });
    expect(resumed).toMatchObject({ status: "ACTIVE", currentBatchId: undefined, queuedBatchIds: ["batch-b"] });
    expect(resumed.pauseReason).toBeUndefined();
  });

  it("recognizes clean completion and structured queue busy errors", () => {
    expect(isCleanSequentialCompletion(batch())).toBe(true);
    expect(isCleanSequentialCompletion(batch({ failed: 1, succeeded: 0 }))).toBe(false);
    expect(isStructuredProductionQueueBusy({ code: "PRODUCTION_QUEUE_BUSY" })).toBe(true);
    expect(isStructuredProductionQueueBusy({ code: "COMFY_TIMEOUT" })).toBe(false);
  });
});
