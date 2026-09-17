import { describe, expect, it } from "vitest";
import { productionBatchOutcomeLabel } from "./productionQueueOutcome";

describe("productionBatchOutcomeLabel", () => {
  it("does not call a failed terminal batch successful", () => {
    expect(productionBatchOutcomeLabel({
      status: "COMPLETED",
      total: 1,
      succeeded: 0,
      failed: 1,
      cancelled: 0,
      skipped: 0,
    })).toBe("生成失败 · 1/1 已处理 · 成功 0 项，失败 1 项");
  });

  it("reports a clean completed batch separately", () => {
    expect(productionBatchOutcomeLabel({
      status: "COMPLETED",
      total: 2,
      succeeded: 2,
      failed: 0,
      cancelled: 0,
      skipped: 0,
    })).toBe("全部生成成功 · 2 项");
  });
});
