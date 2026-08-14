import { describe, expect, it } from "vitest";
import type { WorkflowBenchmarkCandidatePreview } from "../../types/benchmark";
import {
  benchmarkAdmissionNotice,
  canRunBenchmarkDraft,
  previewForCandidatePosition,
} from "./workflowBenchmarkUi";

function preview(id: string, position: number, label: string): WorkflowBenchmarkCandidatePreview {
  return {
    id,
    position,
    workflowVersionId: `workflow-${position}`,
    recipeId: `recipe-${position}`,
    label,
    compatibility: "COMPATIBLE",
    compatibilityReasons: [],
    frozenValues: {},
    assetIds: [`asset-${position}`],
  };
}

describe("workflow benchmark UI mapping", () => {
  it("maps preview candidates by stable position instead of temporary candidate keys", () => {
    const previews = [preview("bmc_backend-a", 0, "first"), preview("bmc_backend-b", 1, "second")];

    expect(previewForCandidatePosition(previews, 0)?.label).toBe("first");
    expect(previewForCandidatePosition(previews, 1)?.label).toBe("second");
    expect(previewForCandidatePosition(previews, 2)).toBeUndefined();
  });

  it("only exposes Run for an unqueued DRAFT clone", () => {
    expect(canRunBenchmarkDraft("DRAFT")).toBe(true);
    expect(canRunBenchmarkDraft("DRAFT", "pbt_existing")).toBe(false);
    expect(canRunBenchmarkDraft("QUEUED")).toBe(false);
  });

  it("reports the returned admission state without claiming a false start", () => {
    expect(benchmarkAdmissionNotice("RUNNING", true)).toContain("已开始");
    expect(benchmarkAdmissionNotice("QUEUED", true)).toContain("繁忙");
    expect(benchmarkAdmissionNotice("QUEUED", true)).not.toContain("已开始");
  });
});
