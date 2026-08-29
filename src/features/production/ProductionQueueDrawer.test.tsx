import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
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
    expect(html).toContain('data-action="start"');
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

    expect(html).toContain('aria-label="开始队列 batch-ready"');
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
});
