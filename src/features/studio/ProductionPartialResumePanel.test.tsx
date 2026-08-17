import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getProductionPartialResumePlan,
  partialResumeProductionQueue,
} from "../../services/tauriClient";
import type { ProductionPartialResumePlan } from "../../types/productionQueue";
import {
  PartialResumePreview,
  defaultPartialResumeSelection,
} from "./ProductionPartialResumePanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const plan: ProductionPartialResumePlan = {
  batchId: "batch-1",
  logicalTotal: 3,
  attemptTotal: 4,
  resolved: 1,
  autoResumable: 1,
  reviewRequired: 1,
  pending: 0,
  active: 0,
  canResume: true,
  entries: [
    {
      rootItemId: "root-1",
      leafItemId: "leaf-1",
      ordinal: 0,
      attemptCount: 2,
      status: "AUTO_RESUMABLE",
      taskId: "task-1",
      errorCode: "COMFY_TIMEOUT",
      errorMessage: "timeout",
      eligibility: "AUTO_RESUMABLE",
    },
    {
      rootItemId: "root-2",
      leafItemId: "leaf-2",
      ordinal: 1,
      attemptCount: 1,
      status: "REVIEW_REQUIRED",
      taskId: "task-2",
      errorCode: "EXECUTION_ERROR",
      errorMessage: "failed",
      eligibility: "REVIEW_REQUIRED",
    },
  ],
};

describe("failed batch partial resume frontend contract", () => {
  it("uses the plan and execute Tauri commands with the queue request envelope", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(plan);
    await getProductionPartialResumePlan("project-1", "batch-1");
    expect(invoke).toHaveBeenLastCalledWith("production_queue_partial_resume_plan", {
      projectId: "project-1",
      batchId: "batch-1",
    });

    vi.mocked(invoke).mockResolvedValueOnce({});
    await partialResumeProductionQueue("project-1", "batch-1", ["leaf-1"]);
    expect(invoke).toHaveBeenLastCalledWith("production_queue_partial_resume", {
      projectId: "project-1",
      batchId: "batch-1",
      selectedLeafItemIds: ["leaf-1"],
    });
  });

  it("checks AUTO_RESUMABLE entries by default and keeps REVIEW_REQUIRED read-only", () => {
    expect(defaultPartialResumeSelection(plan)).toEqual(["leaf-1"]);
    const html = renderToStaticMarkup(
      <PartialResumePreview
        plan={plan}
        selectedLeafItemIds={["leaf-1"]}
        onToggle={vi.fn()}
        onConfirm={vi.fn()}
        busy={false}
      />,
    );

    expect(html).toContain("失败项恢复");
    expect(html).toContain("逻辑任务：<strong>3</strong>");
    expect(html).toContain("已完成：<strong>1</strong>");
    expect(html).toContain("可恢复：<strong>1</strong>");
    expect(html).toContain("需人工检查：<strong>1</strong>");
    expect(html).toContain("Attempts：<strong>4</strong>");
    expect(html).toContain('checked=""');
    expect(html).toContain("需人工检查");
    expect(html).toContain('disabled=""');
  });

  it("renders a disabled confirm action while resume is busy", () => {
    const html = renderToStaticMarkup(
      <PartialResumePreview
        plan={plan}
        selectedLeafItemIds={["leaf-1"]}
        onToggle={vi.fn()}
        onConfirm={vi.fn()}
        busy
      />,
    );

    expect(html).toContain("正在恢复…");
    expect(html).toContain('button type="button" disabled=""');
  });
});
