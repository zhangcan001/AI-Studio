// @vitest-environment jsdom

import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import type { AssetView } from "../../types/asset";
import type { ProductionBatchReviewProductivity, ProductionReviewProductivityItem } from "../../services/tauriClient";
import { regenerateProductionItem } from "../../services/tauriClient";
import { ShotBatchReviewBoard, isVideoReviewReworkAvailable, matchesFilter, reviewCounts, reviewImageIdsForItem, toCompareItem, toLocalCompareItem } from "./ShotBatchReviewBoard";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const asset = (id: string, kind: "image" | "video" = "image"): AssetView => ({
  id,
  assetType: kind,
  category: kind === "video" ? "generated_video" : "generated_image",
  name: id,
  originalName: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png",
  fileSize: 1,
  createdAt: "2026-08-27T00:00:00Z",
  isFavorite: false,
  tags: [],
});

const candidate = (id: string, kind: "image" | "video" = "image") => ({
  assetId: id,
  assetType: kind,
  name: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png",
  thumbnailAvailable: false,
  taskId: "task-1",
  selected: false,
});

const reviewItem = (overrides: Partial<ProductionReviewProductivityItem> = {}): ProductionReviewProductivityItem => ({
  itemId: "item-1",
  ordinal: 0,
  taskId: "task-1",
  taskStatus: "SUCCEEDED",
  productionItemStatus: "SUCCEEDED",
  reviewStatus: "UNREVIEWED",
  reviewNote: "",
  preferred: false,
  workflowVersionId: "workflow-1",
  recipeId: "recipe-1",
  qualityProfile: "QUALITY",
  createdAt: "2026-08-27T00:00:00Z",
  outputAssets: [asset("asset-a")],
  shotId: "shot-1",
  stage: "IMAGE",
  selectedAssetId: undefined,
  reviewable: true,
  candidateAssets: [candidate("asset-a")],
  context: { shotId: "shot-1", stage: "IMAGE", snapshotAvailable: true, promptText: "prompt", referenceSets: [], referenceAssets: [] },
  ...overrides,
});

const reviewBatch = (items: ProductionReviewProductivityItem[]): ProductionBatchReviewProductivity => ({
  batch: {} as never,
  total: items.length,
  successCount: items.filter((item) => item.productionItemStatus === "SUCCEEDED").length,
  failedCount: items.filter((item) => item.productionItemStatus === "FAILED").length,
  unreviewedCount: items.filter((item) => item.reviewStatus === "UNREVIEWED").length,
  approvedCount: items.filter((item) => item.reviewStatus === "APPROVED").length,
  starredCount: items.filter((item) => item.reviewStatus === "STARRED").length,
  regenerateCount: items.filter((item) => item.reviewStatus === "REGENERATE").length,
  rejectedCount: items.filter((item) => item.reviewStatus === "REJECTED").length,
  items,
});

const renderReviewBoard = async (
  items: ProductionReviewProductivityItem[],
  options: Partial<React.ComponentProps<typeof ShotBatchReviewBoard>> = {},
) => {
  const loader = options.reviewBatchLoader ?? vi.fn(async () => reviewBatch(items));
  const rendered = render(
    <ShotBatchReviewBoard
      projectId="project-1"
      shots={[]}
      assets={[]}
      stage={options.stage ?? "image"}
      busy={false}
      onAssetsLoaded={vi.fn()}
      onSelect={vi.fn()}
      onRetry={vi.fn()}
      reviewBatchId="batch-1"
      reviewBatchLoader={loader}
      {...options}
    />,
  );
  await waitFor(() => expect(screen.getByLabelText("审核动作")).toBeTruthy());
  return { rendered, loader };
};

const originalConfirm = window.confirm;
const createObjectUrl = vi.fn(() => "blob:review-asset");
const revokeObjectUrl = vi.fn();

beforeAll(() => {
  Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: createObjectUrl });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: revokeObjectUrl });
});

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(new ArrayBuffer(0));
  createObjectUrl.mockClear();
  revokeObjectUrl.mockClear();
  window.confirm = originalConfirm;
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.confirm = originalConfirm;
});

afterAll(() => {
  delete (URL as unknown as { createObjectURL?: unknown }).createObjectURL;
  delete (URL as unknown as { revokeObjectURL?: unknown }).revokeObjectURL;
});

describe("ShotBatchReviewBoard adapter", () => {
  it("keeps the legacy image/video controls and does not invoke callbacks during render", () => {
    const onSelect = vi.fn();
    const imageShot = { id: "shot-1", ordinal: 0, name: "镜头 1", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }], status: "READY", imageStatus: "READY", videoStatus: "NOT_STARTED" } as never;
    const videoShot = { id: "shot-2", ordinal: 1, name: "镜头 2", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "video", task: { outputAssetIds: ["asset-v"] } }], status: "READY", imageStatus: "READY", videoStatus: "READY" } as never;
    const common = { projectId: "project-1", assets: [asset("asset-a"), asset("asset-v", "video")], busy: false, onAssetsLoaded: vi.fn(), onSelect, onRetry: vi.fn() };
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("设为关键帧");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[videoShot]} stage="video" />)).toContain("设为最终视频");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("打开 A/B 对比");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("builds local A/B items from loaded assets without review status or context", () => {
    const shot = { id: "shot-local", ordinal: 2, name: "本地镜头", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }] } as never;
    const local = toLocalCompareItem(shot, [asset("asset-a")], "image", "project-1");
    expect(local.candidates).toHaveLength(1);
    expect(local.reviewStatus).toBeUndefined();
    expect(local.context).toBeUndefined();
    expect(local.candidates[0].selected).toBe(false);
  });

  it("maps the enhanced payload without fetching candidate metadata and separates status from Shot selection", () => {
    const item = reviewItem({ selectedAssetId: "asset-a", reviewStatus: "APPROVED" });
    const mapped = toCompareItem(item, "project-1", { "asset-a": "blob:asset-a" });
    expect(mapped.candidates[0]).toMatchObject({ id: "asset-a", imageUrl: "blob:asset-a", selected: false });
    expect(mapped.selectedCandidateId).toBe("asset-a");
    expect(mapped.reviewStatus).toBe("APPROVED");
    expect(mapped.contextSnapshot?.contextHash).toBeUndefined();
  });

  it("keeps filter counts and matches every public review status", () => {
    const items = [reviewItem(), reviewItem({ itemId: "item-2", reviewStatus: "APPROVED" }), reviewItem({ itemId: "item-3", reviewStatus: "STARRED" }), reviewItem({ itemId: "item-4", reviewStatus: "REGENERATE" }), reviewItem({ itemId: "item-5", reviewStatus: "REJECTED" }), reviewItem({ itemId: "item-6", reviewStatus: "REGENERATE", taskStatus: "FAILED", productionItemStatus: "FAILED" })];
    expect(reviewCounts(items)).toEqual({ unreviewed: 1, approved: 1, starred: 1, regenerate: 2, rejected: 1, failed: 1 });
    expect(matchesFilter(items[0], "UNREVIEWED")).toBe(true);
    expect(matchesFilter(items[1], "APPROVED")).toBe(true);
    expect(matchesFilter(items[2], "STARRED")).toBe(true);
    expect(matchesFilter(items[3], "REGENERATE")).toBe(true);
    expect(matchesFilter(items[4], "REJECTED")).toBe(true);
    expect(matchesFilter(items[5], "FAILED")).toBe(true);
    expect(matchesFilter(items[0], "NEEDS_REVIEW" as never)).toBe(false);
  });

  it("uses real DOM filters to change the visible review item", async () => {
    const items = [
      reviewItem({ itemId: "unreviewed", shotId: "shot-unreviewed", stage: "VIDEO", outputAssets: [asset("unreviewed-video", "video")], candidateAssets: [candidate("unreviewed-video", "video")] }),
      reviewItem({ itemId: "approved", shotId: "shot-approved", stage: "VIDEO", reviewStatus: "APPROVED", outputAssets: [asset("approved-video", "video")], candidateAssets: [candidate("approved-video", "video")] }),
      reviewItem({ itemId: "starred", shotId: "shot-starred", stage: "VIDEO", reviewStatus: "STARRED", outputAssets: [asset("starred-video", "video")], candidateAssets: [candidate("starred-video", "video")] }),
      reviewItem({ itemId: "regenerate", shotId: "shot-regenerate", stage: "VIDEO", reviewStatus: "REGENERATE", outputAssets: [asset("regenerate-video", "video")], candidateAssets: [candidate("regenerate-video", "video")] }),
      reviewItem({ itemId: "rejected", shotId: "shot-rejected", stage: "VIDEO", reviewStatus: "REJECTED", outputAssets: [asset("rejected-video", "video")], candidateAssets: [candidate("rejected-video", "video")] }),
      reviewItem({ itemId: "failed", shotId: "shot-failed", stage: "VIDEO", reviewStatus: "FAILED", taskStatus: "FAILED", productionItemStatus: "FAILED", outputAssets: [asset("failed-video", "video")], candidateAssets: [candidate("failed-video", "video")] }),
    ];
    const user = userEvent.setup();
    await renderReviewBoard(items, { stage: "video" });
    for (const label of ["全部 6", "未审核 1", "已通过 1", "标星 1", "待返工 1", "已拒绝 1", "失败 1"]) expect(screen.getByRole("button", { name: label })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "已通过 1" }));
    expect(await screen.findByRole("heading", { name: "shot-approved" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "shot-unreviewed" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "标星 1" }));
    expect(await screen.findByRole("heading", { name: "shot-starred" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "已拒绝 1" }));
    expect(await screen.findByRole("heading", { name: "shot-rejected" })).toBeTruthy();
  });

  it("requires explicit confirmation before creating an eligible video rework batch", async () => {
    const item = reviewItem({ itemId: "video-item", shotId: "video-shot", stage: "VIDEO", outputAssets: [asset("video-a", "video")], candidateAssets: [candidate("video-a", "video")] });
    const onOpenProductionQueue = vi.fn();
    await renderReviewBoard([item], { stage: "video", onOpenProductionQueue });
    const confirm = vi.fn(() => false);
    window.confirm = confirm;
    await userEvent.setup().click(screen.getByRole("button", { name: "创建返工批次" }));
    expect(confirm).toHaveBeenCalledWith("确定创建返工批次吗？\n创建后不会自动开始，仍需前往生产队列手动启动。");
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("production_queue_start", expect.anything());
    expect(onOpenProductionQueue).not.toHaveBeenCalled();
  });

  it("creates a confirmed video rework with autoStart false and never starts the queue", async () => {
    const item = reviewItem({ itemId: "video-item", shotId: "video-shot", stage: "VIDEO", outputAssets: [asset("video-a", "video")], candidateAssets: [candidate("video-a", "video")] });
    const onOpenProductionQueue = vi.fn();
    await renderReviewBoard([item], { stage: "video", onOpenProductionQueue });
    window.confirm = vi.fn(() => true);
    vi.mocked(invoke).mockResolvedValue({ selectedCount: 1 });
    await userEvent.setup().click(screen.getByRole("button", { name: "创建返工批次" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("production_item_review_regenerate", { request: expect.objectContaining({ itemId: "video-item", autoStart: false }) }));
    expect(invoke).not.toHaveBeenCalledWith("production_queue_start", expect.anything());
    expect(onOpenProductionQueue).toHaveBeenCalledOnce();
  });

  it("disables review rework for image items and preserves the legacy retry boundary", async () => {
    const item = reviewItem({ itemId: "image-item", shotId: "image-shot", stage: "IMAGE" });
    const onRetry = vi.fn();
    await renderReviewBoard([item], { stage: "image", onRetry });
    const createButton = screen.getByRole("button", { name: "创建返工批次" });
    expect(isVideoReviewReworkAvailable(item)).toBe(false);
    expect((createButton as HTMLButtonElement).disabled).toBe(true);
    vi.mocked(invoke).mockClear();
    await userEvent.setup().click(createButton);
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate_marked", expect.anything());
    expect(onRetry).not.toHaveBeenCalled();
  });

  it("requires Shot selection before APPROVED and stops on selection failure", async () => {
    const item = reviewItem({ itemId: "video-item", shotId: "video-shot", stage: "VIDEO", outputAssets: [asset("video-a", "video")], candidateAssets: [candidate("video-a", "video")] });
    const calls: string[] = [];
    vi.mocked(invoke).mockImplementation(async (command) => {
      calls.push(command);
      return {};
    });
    await renderReviewBoard([item], { stage: "video" });
    await userEvent.setup().click(screen.getByRole("button", { name: "确认并通过" }));
    await waitFor(() => expect(calls).toEqual(["shot_result_select", "production_item_review_set_status"]));

    cleanup();
    calls.length = 0;
    vi.mocked(invoke).mockImplementation(async (command) => {
      calls.push(command);
      if (command === "shot_result_select") throw new Error("selection rejected");
      return {};
    });
    await renderReviewBoard([item], { stage: "video" });
    await userEvent.setup().click(screen.getByRole("button", { name: "确认并通过" }));
    await waitFor(() =>
      expect(
        screen.getAllByRole("alert").some((element) => element.textContent?.includes("selection rejected")),
      ).toBe(true),
    );
    expect(calls).toEqual(["shot_result_select"]);
    expect(calls).not.toContain("production_item_review_set_status");
  });

  it("keeps the first item after approval and reports partial status failure without auto-next", async () => {
    const first = reviewItem({ itemId: "item-1", shotId: "shot-1", stage: "VIDEO", outputAssets: [asset("video-1", "video")], candidateAssets: [candidate("video-1", "video")] });
    const second = reviewItem({ itemId: "item-2", ordinal: 1, shotId: "shot-2", stage: "VIDEO", outputAssets: [asset("video-2", "video")], candidateAssets: [candidate("video-2", "video")] });
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "production_item_review_set_status") throw new Error("status rejected");
      return {};
    });
    const user = userEvent.setup();
    await renderReviewBoard([first, second], { stage: "video" });
    await user.click(screen.getByRole("button", { name: "确认并通过" }));
    await waitFor(() =>
      expect(
        screen
          .getAllByRole("alert")
          .some((element) => element.textContent?.includes("候选已设为采用结果，但审片状态未更新，请重新点击通过。")),
      ).toBe(true),
    );
    expect(screen.getByRole("heading", { name: "shot-1" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "下一项" }));
    expect(screen.getByRole("heading", { name: "shot-2" })).toBeTruthy();
  });

  it("limits review image reads to the current item instead of the whole batch", () => {
    const current = reviewItem({ itemId: "current", candidateAssets: [candidate("current-a"), candidate("current-video", "video")] });
    const other = reviewItem({ itemId: "other", candidateAssets: [candidate("other-a")] });
    expect(reviewImageIdsForItem(current)).toEqual(["current-a"]);
    expect(reviewImageIdsForItem(other)).toEqual(["other-a"]);
    expect(reviewImageIdsForItem(undefined)).toEqual([]);
  });

  it("forces regeneration payload autoStart false while retaining the wire field", async () => {
    vi.mocked(invoke).mockResolvedValue({});
    await regenerateProductionItem({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", useOriginalSeed: false, autoStart: true });
    expect(invoke).toHaveBeenCalledWith("production_item_review_regenerate", { request: expect.objectContaining({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", autoStart: false }) });
  });
});
