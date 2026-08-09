import { describe, expect, it } from "vitest";
import type { TaskDetail } from "../../types/history";
import { taskRetryDecision } from "./retryPolicy";

function failedDetail(errorCode: string): TaskDetail {
  return {
    id: "tsk_1",
    projectId: "prj_default",
    workflowId: "wfl_test",
    workflowVersionId: "wfv_test",
    recipeId: "rcp_test",
    workflowName: "Test",
    status: "FAILED",
    createdAt: "2026-08-07T00:00:00Z",
    errorCode,
    outputAssets: [],
    reusableDraft: { available: true, missingAssetIds: [] },
  };
}

describe("task retry policy", () => {
  it("allows an explicit retry for transient ComfyUI failures when inputs are reusable", () => {
    expect(taskRetryDecision(failedDetail("COMFY_TIMEOUT"), true)).toEqual({ allowed: true });
    expect(taskRetryDecision(failedDetail("COMFY_STREAM_DISCONNECTED"), true)).toEqual({ allowed: true });
  });

  it("never quick-retries execution errors such as MiniMax H3 OOM", () => {
    const decision = taskRetryDecision(failedDetail("EXECUTION_ERROR"), true);
    expect(decision.allowed).toBe(false);
    expect(decision.reason).toContain("显存不足");
  });

  it("blocks retry when ComfyUI is offline or source media is missing", () => {
    expect(taskRetryDecision(failedDetail("COMFY_TIMEOUT"), false).allowed).toBe(false);
    const missing = failedDetail("COMFY_TIMEOUT");
    missing.reusableDraft.missingAssetIds = ["ast_missing"];
    expect(taskRetryDecision(missing, true).allowed).toBe(false);
  });
});
