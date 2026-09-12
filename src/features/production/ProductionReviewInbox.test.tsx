// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { getProductionReviewInbox } from "../../services/tauriClient";
import { ProductionReviewInbox } from "./ProductionReviewInbox";

vi.mock("../../services/tauriClient", () => ({ getProductionReviewInbox: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("shows the selected result and navigates to its exact Asset, with IDs folded away", async () => {
  vi.mocked(getProductionReviewInbox).mockResolvedValue({
    items: [{
      projectId: "project-1", batchId: "batch-1", batchName: "开场批次", batchStatus: "READY",
      itemId: "item-1", ordinal: 0, itemStatus: "COMPLETED", taskId: "task-1",
      shotId: "shot-1", stage: "VIDEO", assetId: "output-1", selectedAssetId: "selected-1",
      reviewStatus: "APPROVED", reviewNote: "", version: 1,
      workflowVersionId: "wfv-1", recipeId: "recipe-1", updatedAt: "2026-09-12T00:00:00Z",
    }], total: 1, unreviewedCount: 0, regenerateCount: 0, limit: 25, offset: 0,
  });
  const onNavigate = vi.fn();
  render(<ProductionReviewInbox projectId="project-1" onNavigate={onNavigate} />);
  expect(await screen.findByText("已选最终结果 · 可打开素材查看")).toBeTruthy();
  expect(screen.getByText("待审核结果")).toBeTruthy();
  expect(screen.getByText(/批次 batch-1/).closest("details")?.open).toBe(false);
  await userEvent.setup().click(screen.getByRole("button", { name: "打开审片" }));
  expect(onNavigate).toHaveBeenCalledWith({
    destination: "shots",
    section: "review",
    batchId: "batch-1",
    itemId: "item-1",
    reviewId: "item-1",
    taskId: "task-1",
    shotId: "shot-1",
    stage: "VIDEO",
  });
  await userEvent.setup().click(screen.getByRole("button", { name: "最终结果" }));
  expect(onNavigate).toHaveBeenCalledWith({ destination: "assets", assetId: "selected-1" });
});
