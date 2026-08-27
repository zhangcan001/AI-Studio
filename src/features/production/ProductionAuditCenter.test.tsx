// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProductionAuditCenterView, filterActivities } from "./ProductionAuditCenter";
import type {
  ProductionAuditActivity,
  ProductionAuditLineage,
  ProductionAuditSummary,
} from "../../types/productionAudit";

const summary: ProductionAuditSummary = {
  projectId: "project-1",
  health: "HEALTHY",
  activeRuns: 1,
  completedRuns: 4,
  failedRuns: 0,
  activeBatches: 1,
  pausedBatches: 0,
  failedBatches: 0,
  logicalItems: 3,
  attempts: 4,
  succeededItems: 2,
  failedItems: 1,
  reviewRequiredItems: 1,
  tasks: 4,
  succeededTasks: 3,
  failedTasks: 1,
  assets: 3,
  unassignedShots: 0,
  checkedAt: "2026-08-18T10:00:00Z",
  issues: [],
};

const activity: ProductionAuditActivity[] = [
  {
    id: "activity-retry",
    kind: "ITEM_RETRIED",
    timestamp: "2026-08-18T09:00:00Z",
    severity: "WARNING",
    title: "逻辑项已重试",
    detail: "B → B2",
    batchId: "batch-1",
    itemId: "item-b2",
    retryOfItemId: "item-b",
  },
  {
    id: "activity-task",
    kind: "TASK_SUCCEEDED",
    timestamp: "2026-08-18T08:00:00Z",
    severity: "INFO",
    title: "Task 已完成",
    detail: "佛陀端坐",
    taskId: "task-a",
    shotId: "shot-a",
    shotName: "佛陀端坐",
  },
  {
    id: "activity-failed",
    kind: "TASK_FAILED",
    timestamp: "2026-08-18T07:00:00Z",
    severity: "ERROR",
    title: "Task 失败",
    detail: "Comfy timeout",
    taskId: "task-c",
    errorCode: "COMFY_TIMEOUT",
    status: "FAILED",
  },
];

const lineage: ProductionAuditLineage = {
  projectId: "project-1",
  rootType: "BATCH",
  rootId: "batch-1",
  nodes: [
    { id: "batch-1", entityType: "BATCH", label: "Batch 1" },
    { id: "logical-b", entityType: "LOGICAL_ITEM", label: "B", parentId: "batch-1" },
    { id: "attempt-b2", entityType: "ATTEMPT", label: "B2", parentId: "logical-b" },
    { id: "task-b2", entityType: "TASK", label: "Task B2", taskId: "task-b2", parentId: "attempt-b2" },
    { id: "snapshot-b2", entityType: "SNAPSHOT", label: "Snapshot B2", parentId: "task-b2" },
    { id: "asset-b2", entityType: "ASSET", label: "Asset B2", assetId: "asset-b2", parentId: "task-b2" },
  ],
};

afterEach(cleanup);

describe("ProductionAuditCenter", () => {
  it.each(["HEALTHY", "WARNING", "BLOCKED"] as const)("renders %s health", (health) => {
    const html = renderToStaticMarkup(<ProductionAuditCenterView summary={{ ...summary, health }} />);
    expect(html).toContain(`生产数据健康：${health === "HEALTHY" ? "健康" : health === "WARNING" ? "需要关注" : "已阻断"}`);
  });

  it("renders summary cards and recent activity with task reuse affordance", () => {
    const html = renderToStaticMarkup(
      <ProductionAuditCenterView
        summary={summary}
        activity={activity}
        onOpenTask={vi.fn()}
        onOpenShot={vi.fn()}
        onInspectLineage={vi.fn()}
      />,
    );
    expect(html).toContain("生产审计");
    expect(html).toContain("待人工检查");
    expect(html).toContain("最近活动");
    expect(html).toContain("COMFY_TIMEOUT");
    expect(html).toContain("任务");
    expect(html).toContain("查看镜头");
    expect(html).not.toContain("Task Detail 2");
  });

  it("filters failed and retry activity without changing persisted data", () => {
    expect(filterActivities(activity, "FAILED", "timeout").map((item) => item.id)).toEqual(["activity-failed"]);
    expect(filterActivities(activity, "RETRIED", "").map((item) => item.id)).toEqual(["activity-retry"]);
    expect(filterActivities(activity, "RUNNING", "")).toEqual([]);
  });

  it("renders the ordered lineage tree and links back to task/shot workspaces", () => {
    const html = renderToStaticMarkup(
      <ProductionAuditCenterView
        summary={summary}
        lineage={lineage}
        onOpenTask={vi.fn()}
        onOpenShot={vi.fn()}
      />,
    );
    expect(html).toContain("运行 → 阶段 → 批次 → 逻辑项 → 尝试 → 任务 → 快照 → 素材");
    expect(html).toContain("逻辑项");
    expect(html).toContain("Task B2");
    expect(html).toContain("Snapshot B2");
    expect(html).toContain("Asset B2");
    expect(html).toContain("任务详情");
  });

  it("shows a useful empty state when no audit data exists", () => {
    const html = renderToStaticMarkup(<ProductionAuditCenterView />);
    expect(html).toContain("当前项目暂无生产审计数据");
    expect(html).toContain("当前筛选条件下没有审计活动");
    expect(html).toContain("选择最近活动或输入对象 ID 查看生产链路");
  });

  it("shows preparation snapshot lineage and loads detail only after expansion", async () => {
    const user = userEvent.setup();
    const onLoadSnapshotDetail = vi.fn();
    const onCopyContextHash = vi.fn();
    const snapshotId = "pps-1";
    const contextHash = "context-hash-2026-08-18";
    const preparationLineage: ProductionAuditLineage = {
      ...lineage,
      nodes: [
        { id: "batch-1", entityType: "BATCH", label: "Batch 1" },
        { id: snapshotId, entityType: "PREPARATION_SNAPSHOT", label: "生产准备快照", snapshotId, contextHash, snapshotSchemaVersion: 1, stage: "IMAGE", batchId: "batch-1", itemId: "item-1", createdAt: "2026-08-18T09:30:00Z", parentId: "batch-1" },
      ],
    };
    const { rerender, container } = render(
      <ProductionAuditCenterView
        summary={summary}
        lineage={preparationLineage}
        onLoadSnapshotDetail={onLoadSnapshotDetail}
        onCopyContextHash={onCopyContextHash}
        snapshotDetails={{}}
      />,
    );

    expect(container.textContent).toContain("生产准备快照");
    expect(container.textContent).toContain(contextHash);
    expect(container.textContent).toContain("IMAGE");
    expect(container.textContent).toContain("item-1");
    expect(container.textContent).not.toContain("冻结提示词");

    await user.click(screen.getByRole("button", { name: "查看快照详情" }));
    expect(onLoadSnapshotDetail).toHaveBeenCalledWith(expect.objectContaining({ id: snapshotId }));

    rerender(
      <ProductionAuditCenterView
        summary={summary}
        lineage={preparationLineage}
        onLoadSnapshotDetail={onLoadSnapshotDetail}
        onCopyContextHash={onCopyContextHash}
        snapshotDetails={{ [snapshotId]: { id: snapshotId, projectId: "project-1", shotId: "shot-1", stage: "IMAGE", contextHash, snapshotSchemaVersion: 1, productionBatchId: "batch-1", productionBatchItemId: "item-1", createdAt: "2026-08-18T09:30:00Z", prompt: "冻结提示词", negativePrompt: "negative", workflowVersionId: "workflow-v1", recipeId: "recipe-v1", referenceSetIds: ["rs-1"], assetChecksums: ["sha-1"] } }}
      />,
    );
    expect(container.textContent).toContain("冻结提示词");
    expect(container.textContent).toContain("workflow-v1");
    expect(container.textContent).toContain("rs-1");

    await user.click(screen.getByRole("button", { name: "复制 contextHash" }));
    expect(onCopyContextHash).toHaveBeenCalledWith(contextHash);
  });

  it("labels historical lineage without a preparation snapshot as legacy instead of an error", () => {
    const { container } = render(<ProductionAuditCenterView summary={summary} lineage={lineage} />);
    expect(container.textContent).toContain("旧版生产记录，无准备快照");
    expect(container.textContent).not.toContain("生产准备快照缺失");
  });
});
