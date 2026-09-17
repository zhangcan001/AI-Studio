// @vitest-environment jsdom

import { cleanup, render } from "@testing-library/react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi, afterEach } from "vitest";
import type { ProductionBatchDetail, ProductionBatchItemView, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import { ProductionQueueDrawer } from "./ProductionQueueDrawer";

const overview: ProductionQueueOverview = {
  totalQueues: 3,
  runningQueues: 3,
  pausedQueues: 0,
  completedQueues: 0,
  archivedQueues: 0,
  totalItems: 13,
  pendingItems: 12,
  activeItems: 1,
  succeededItems: 0,
  failedItems: 1,
  cancelledItems: 0,
  skippedItems: 0,
};

const item = (overrides: Partial<ProductionBatchItemView> = {}): ProductionBatchItemView => ({
  id: "item-1",
  ordinal: 0,
  workflowVersionId: "workflow-1",
  recipeId: "recipe-1",
  status: "FAILED",
  promptText: "雨夜巷口，火光映照湿润石板路",
  ...overrides,
});

const detail = (overrides: Partial<ProductionBatchDetail> = {}): ProductionBatchDetail => ({
  id: "batch-1",
  projectId: "project-1",
  name: "第一季 · 第01集 · Scene 04",
  status: "RUNNING",
  continueOnFailure: false,
  createdAt: "2026-08-25T00:00:00Z",
  updatedAt: "2026-08-25T00:00:00Z",
  total: 10,
  pending: 4,
  running: 1,
  succeeded: 5,
  failed: 0,
  cancelled: 0,
  skipped: 0,
  items: [item({ status: "DISPATCHED" })],
  ...overrides,
});

const runbook: ProductionBatchRunbookView = {
  projectId: "project-1",
  rows: [{
    batchId: "batch-runbook",
    batchName: "第一季 · 第01集 · Scene 05",
    batchStatus: "READY",
    stage: "image",
    shotCount: 8,
    pending: 8,
    active: 0,
    succeeded: 0,
    failed: 0,
    createdAt: "2026-08-25T00:00:00Z",
    readyToStart: true,
  }],
  summary: {
    batchTotal: 1,
    readyBatches: 1,
    runningBatches: 0,
    pausedBatches: 0,
    completedBatches: 0,
    pending: 8,
    active: 0,
    succeeded: 0,
    failed: 0,
  },
};

describe("ProductionQueueDrawer", () => {
  afterEach(() => {
    cleanup();
  });

  it("is collapsed by default and shows real overview counts without rendering rows", () => {
    const html = renderToStaticMarkup(<ProductionQueueDrawer overview={overview} details={[detail()]} />);

    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("生产队列");
    expect(html).toContain("3");
    expect(html).toContain("12");
    expect(html).toContain("1");
    expect(html).not.toContain("第一季 · 第01集 · Scene 04");
  });

  it("supports the expanded state and uses runbook summary when queue overview is absent", () => {
    const html = renderToStaticMarkup(<ProductionQueueDrawer runbook={runbook} defaultExpanded onStart={vi.fn()} />);

    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain("第一季 · 第01集 · Scene 05");
    expect(html).toContain("图片");
    expect(html).toContain("8");
    expect(html).toContain("只有在这里点击“开始生产”才会创建并提交真实生产任务");
    expect(html).toContain('data-action="start"');
  });

  it("honors controlled expansion changes from collapsed to expanded", () => {
    const { rerender } = render(<ProductionQueueDrawer expanded={false} details={[detail()]} />);

    const drawer = document.querySelector("[aria-label='生产队列']") as HTMLElement;
    const toggle = drawer.querySelector("button[aria-controls]") as HTMLButtonElement;
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(drawer.querySelector(".production-queue-drawer-body")).toBeNull();

    rerender(<ProductionQueueDrawer expanded details={[detail()]} />);

    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(drawer.querySelector(".production-queue-drawer-body")).not.toBeNull();
    expect(drawer.textContent).toContain("第一季 · 第01集 · Scene 04");
  });

  it("renders real detail and item rows plus optional display slots", () => {
    const realItem = item({ status: "FAILED", id: "item-failed", ordinal: 2 }) as ProductionBatchItemView & {
      shotName: string;
      stage: string;
      resolution: string;
      duration: string;
    };
    realItem.shotName = "Shot 04 业火焚烧";
    realItem.stage = "video";
    realItem.resolution = "1280×720";
    realItem.duration = "8s";

    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        details={[detail()]}
        items={[realItem]}
        defaultExpanded
        onPause={vi.fn()}
        onRetry={vi.fn()}
        onOpen={vi.fn()}
      />,
    );

    expect(html).toContain("第一季 · 第01集 · Scene 04");
    expect(html).toContain("Shot 04 业火焚烧");
    expect(html).toContain("视频");
    expect(html).toContain("1280×720");
    expect(html).toContain("8s");
    expect(html).toContain('data-action="pause"');
    expect(html).toContain('data-action="retry"');
    expect(html).toContain('data-action="open"');
  });

  it("keeps actions item-scoped and does not render a global start control", () => {
    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        details={[detail(), detail({ id: "batch-ready", name: "第二批 · READY", status: "READY", items: [] })]}
        defaultExpanded
        onStart={vi.fn()}
        onPause={vi.fn()}
        onRetry={vi.fn()}
        onOpen={vi.fn()}
      />,
    );

    expect(html).toContain('aria-label="开始生产队列 batch-ready"');
    expect(html).toContain('aria-label="暂停队列 batch-1"');
    expect(html).toContain('aria-label="打开队列 batch-1"');
    expect(html).not.toContain("Start All");
    expect(html).not.toContain("Scheduler");
    expect(html).not.toContain("Auto Start Next");
  });

  it("puts a focused batch first while keeping every queue visible", () => {
    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        queues={[
          { id: "batch-1", projectId: "project-1", name: "第一批", status: "READY", continueOnFailure: false, createdAt: "", updatedAt: "" },
          { id: "batch-2", projectId: "project-1", name: "第二批", status: "READY", continueOnFailure: false, createdAt: "", updatedAt: "" },
          { id: "batch-3", projectId: "project-1", name: "第三批", status: "READY", continueOnFailure: false, createdAt: "", updatedAt: "" },
        ]}
        runbook={{ projectId: "project-1", rows: [] }}
        focusBatchId="batch-2"
        defaultExpanded
      />,
    );

    expect(html).toContain('data-batch-id="batch-2" data-focused="true"');
    expect(html.indexOf('data-batch-id="batch-2"')).toBeLessThan(html.indexOf('data-batch-id="batch-1"'));
    expect(html).toContain('data-batch-id="batch-3"');
  });

  it("marks every batch from the latest quick flow while focusing the first one", () => {
    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        queues={[
          { id: "batch-1", projectId: "project-1", name: "第一批", status: "READY", continueOnFailure: false, createdAt: "", updatedAt: "" },
          { id: "batch-2", projectId: "project-1", name: "第二批", status: "READY", continueOnFailure: false, createdAt: "", updatedAt: "" },
        ]}
        createdBatchIds={["batch-1", "batch-2"]}
        focusBatchId="batch-1"
        defaultExpanded
        onStart={vi.fn()}
      />,
    );

    expect(html).toContain('data-batch-id="batch-1" data-focused="true" data-recently-created="true"');
    expect(html).toContain('data-batch-id="batch-2" data-recently-created="true"');
    expect(html.match(/刚刚创建/g)).toHaveLength(2);
    expect(html).toContain('aria-label="开始生产队列 batch-1"');
    expect(html).toContain('aria-label="开始生产队列 batch-2"');
  });

  it("turns a structured pause reason into actionable Chinese copy with technical details", () => {
    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        sequentialStartStatus="PAUSED"
        sequentialPauseReason="RUNTIME_ADMISSION_COMFY_UNAVAILABLE: ComfyUI status is Offline"
        sequentialCanResume
        onResumeSequentialStart={vi.fn()}
        onOpenSettings={vi.fn()}
      />,
    );

    expect(html).toContain("连续运行已暂停");
    expect(html).toContain("无法启动生产队列：ComfyUI 当前不可用。");
    expect(html).toContain("请先处理运行环境问题，再重试启动");
    expect(html).toContain("重试启动");
    expect(html).toContain("打开连接设置");
    expect(html).toContain("RUNTIME_ADMISSION_COMFY_UNAVAILABLE");
    expect(html).toContain('role="alert"');
  });

  it("does not describe a currently paused batch as a previous-batch failure", () => {
    const html = renderToStaticMarkup(
      <ProductionQueueDrawer
        sequentialStartStatus="PAUSED"
        sequentialPauseReason="当前批次已暂停，请先处理当前批次。"
        sequentialCanResume={false}
      />,
    );

    expect(html).toContain("请在当前批次中继续生产或处理失败项");
    expect(html).not.toContain("请先处理上一批失败项");
  });
});

it("shows zero successes and failure count for a terminal batch", () => {
  const html = renderToStaticMarkup(<ProductionQueueDrawer defaultExpanded details={[detail({ status: "COMPLETED", total: 5, pending: 0, running: 0, succeeded: 0, failed: 5, items: [] })]} />);
  expect(html).toContain("生成失败");
  expect(html).toContain("成功 0 项，失败 5 项");
  expect(html).not.toContain("全部生成成功");
});
