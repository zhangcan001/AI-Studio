import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TaskHistoryDetail } from "./TaskHistoryDetail";
import { taskRetrySubmissionKey } from "./retryPolicy";

describe("任务历史 ComfyUI 校验详情", () => {
  it("uses a stable idempotency key for retry submissions", () => {
    expect(taskRetrySubmissionKey("tsk_failed")).toBe("task-retry:tsk_failed");
    expect(taskRetrySubmissionKey("tsk_failed")).toBe(taskRetrySubmissionKey("tsk_failed"));
  });

  it("renders structured node errors and preserves the raw payload", () => {
    const html = renderToStaticMarkup(
      <TaskHistoryDetail
        projectId="prj_default"
        detail={{
          id: "tsk_failed",
          projectId: "prj_default",
          workflowId: "wfl_h3",
          workflowVersionId: "wfv_h3",
          recipeId: "rcp_h3",
          workflowName: "MiniMax H3",
          status: "FAILED",
          createdAt: "2026-08-14T00:00:00Z",
          errorCode: "WORKFLOW_VALIDATION_FAILED",
          errorMessage: "Node 'NBH3HyperStepSimple' not found.",
          nodeErrors: [
            {
              nodeId: "26",
              nodeType: "NBH3HyperStepSimple",
              input: "mode",
              errorType: "value_not_in_list",
              message: "Value not in list: Middle-36",
              receivedValue: "Middle-36",
            },
          ],
          rawError: { "26": { class_type: "NBH3HyperStepSimple" } },
          outputAssets: [],
          reusableDraft: { available: false, missingAssetIds: [] },
        }}
        loadingDraft={false}
        comfyConnected={true}
        productionBusy={false}
        onBack={vi.fn()}
        onLoadInputs={vi.fn()}
        onOpenAsset={vi.fn()}
      />,
    );

    expect(html).toContain("ComfyUI 节点校验详情");
    expect(html).toContain("节点 26");
    expect(html).toContain("NBH3HyperStepSimple");
    expect(html).toContain("Middle-36");
    expect(html).toContain("展开原始 JSON");
    expect(html).toContain("class_type");
    expect(html).not.toContain("The task did not complete successfully.");
  });
});
