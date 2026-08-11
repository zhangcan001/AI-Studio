import { describe, expect, it } from "vitest";
import type { ProductionBatchItemView } from "../../types/productionQueue";
import { canCancelPendingProductionQueue, isSafeProductionQueueRequeue, productionInteractionPolicy } from "./productionQueuePolicy";

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

describe("pending production queue cancellation policy", () => {
  it("allows cancelling waiting items without active work", () => {
    expect(canCancelPendingProductionQueue({ status: "READY", pending: 1, running: 0 })).toBe(true);
    expect(canCancelPendingProductionQueue({ status: "PAUSED", pending: 2, running: 0 })).toBe(true);
  });

  it("blocks cancellation while running, completed, archived, or empty", () => {
    expect(canCancelPendingProductionQueue({ status: "RUNNING", pending: 1, running: 0 })).toBe(false);
    expect(canCancelPendingProductionQueue({ status: "READY", pending: 1, running: 1 })).toBe(false);
    expect(canCancelPendingProductionQueue({ status: "COMPLETED", pending: 1, running: 0 })).toBe(false);
    expect(canCancelPendingProductionQueue({ status: "READY", pending: 0, running: 0 })).toBe(false);
    expect(canCancelPendingProductionQueue({ status: "READY", pending: 1, running: 0, archivedAt: "2026-08-11T00:00:00Z" })).toBe(false);
  });
});

describe("production interaction admission", () => {
  it("blocks all GPU submission entrances while production is active", () => {
    const policy = productionInteractionPolicy(true);
    expect(policy.canSubmitGeneration).toBe(false);
    expect(policy.canSubmitLocalBatch).toBe(false);
    expect(policy.canRetryTask).toBe(false);
  });

  it("keeps draft editing and project switching available", () => {
    const policy = productionInteractionPolicy(true);
    expect(policy.canEditDraft).toBe(true);
    expect(policy.canSwitchProject).toBe(true);
  });

  it("releases submission after production becomes idle", () => {
    const policy = productionInteractionPolicy(false);
    expect(policy.canSubmitGeneration).toBe(true);
    expect(policy.canSubmitLocalBatch).toBe(true);
    expect(policy.canRetryTask).toBe(true);
  });
});
