// @vitest-environment jsdom
import { render, screen, waitFor } from "@testing-library/react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { RepairJobStatusView } from "../../types/repairJobs";

const jobs: RepairJobStatusView[] = [
  {
    jobId: "recipe_textarea_combo_v1",
    status: "COMPLETED",
    startedAt: "2026-09-28T10:00:00Z",
    completedAt: "2026-09-28T10:00:01Z",
    summary: { planned: 3, repaired: 2, skipped: 1, needsReview: [{ workflowId: "w", workflowVersionId: "v", recipeId: "r", reason: "AMBIGUOUS" }], failed: [], publishedRecipeIds: ["a", "b"] },
    summaryRaw: "{}",
  },
  {
    jobId: "recipe_frame_count_v1",
    status: "FAILED",
    startedAt: "2026-09-28T10:00:01Z",
    completedAt: "2026-09-28T10:00:02Z",
    summary: { planned: 1, repaired: 1, skipped: 0, needsReview: [], failed: [{ recipeId: "x", message: "boom" }], publishedRecipeIds: [] },
    summaryRaw: "{}",
  },
];

vi.mock("../../services/diagnosticsClient", () => ({
  diagnosticsClient: { repairJobs: vi.fn(async () => jobs) },
}));

import { RepairJobsStatusList, RepairJobsStatusSection } from "./RepairJobsStatusSection";

describe("数据修复任务状态", () => {
  it("summarizes repaired recipes and per-job status", () => {
    const html = renderToStaticMarkup(<RepairJobsStatusList jobs={jobs} loading={false} />);
    expect(html).toContain("数据修复任务");
    expect(html).toContain("已自动修复 3 个配方");
    expect(html).toContain("1 项需人工检查");
    expect(html).toContain("recipe_textarea_combo_v1");
    expect(html).toContain("已完成");
    expect(html).toContain("失败 1");
  });

  it("shows an empty state when no job has run", () => {
    const html = renderToStaticMarkup(<RepairJobsStatusList jobs={[]} loading={false} />);
    expect(html).toContain("暂无修复任务记录");
  });

  it("loads status through the read-only command", async () => {
    render(<RepairJobsStatusSection />);
    await waitFor(() => expect(screen.getByText(/已自动修复 3 个配方/)).toBeTruthy());
    expect(screen.queryByRole("button")).toBeNull();
  });
});
