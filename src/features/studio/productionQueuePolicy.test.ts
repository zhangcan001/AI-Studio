import { describe, expect, it } from "vitest";
import type { ProductionBatchItemView } from "../../types/productionQueue";
import { isSafeProductionQueueRequeue } from "./productionQueuePolicy";

function item(status: ProductionBatchItemView["status"], errorCode?: string): ProductionBatchItemView {
  return {
    id: "pbi_test",
    ordinal: 0,
    workflowVersionId: "wfv_test",
    recipeId: "rcp_test",
    status,
    errorCode,
  };
}

describe("production queue requeue policy", () => {
  it("allows cancelled and transient ComfyUI failures", () => {
    expect(isSafeProductionQueueRequeue(item("CANCELLED"))).toBe(true);
    expect(isSafeProductionQueueRequeue(item("FAILED", "COMFY_TIMEOUT"))).toBe(true);
    expect(isSafeProductionQueueRequeue(item("SKIPPED", "COMFY_STREAM_DISCONNECTED"))).toBe(true);
  });

  it("blocks execution errors and uncertain dispatches", () => {
    expect(isSafeProductionQueueRequeue(item("FAILED", "EXECUTION_ERROR"))).toBe(false);
    expect(isSafeProductionQueueRequeue(item("FAILED", "QUEUE_DISPATCH_UNCERTAIN"))).toBe(false);
  });

  it("blocks deterministic failures and non-terminal items", () => {
    expect(isSafeProductionQueueRequeue(item("FAILED", "QUEUE_COMPILE_ERROR"))).toBe(false);
    expect(isSafeProductionQueueRequeue(item("PENDING"))).toBe(false);
    expect(isSafeProductionQueueRequeue(item("SUCCEEDED"))).toBe(false);
  });
});
