import { describe, expect, it } from "vitest";
import {
  assetCategoryLabel,
  assetDisplayName,
  assetTypeLabel,
  comfyStatusLabel,
  formatDateTime,
  projectDisplayName,
  productionItemStatusLabel,
  productionReviewStatusLabel,
  productionStatusLabel,
  taskStatusLabel,
  workflowDisplayName,
  workflowModeLabel,
} from "./statusLabels";
import { errorMessageForCode, formatUiError, toUserMessage } from "./errorMessages";

describe("简体中文状态展示", () => {
  it("maps task and production status values without changing protocol values", () => {
    expect(taskStatusLabel("RUNNING")).toBe("运行中");
    expect(taskStatusLabel("CANCEL_REQUESTED")).toBe("正在取消");
    expect(productionStatusLabel("PAUSED")).toBe("已暂停");
    expect(productionStatusLabel("READY")).toBe("待启动");
    expect(productionStatusLabel("RUNNING")).toBe("运行中");
    expect(productionItemStatusLabel("PENDING")).toBe("待执行");
    expect(productionItemStatusLabel("DISPATCHED")).toBe("运行中");
    expect(productionReviewStatusLabel("UNREVIEWED")).toBe("待审核");
    expect(productionReviewStatusLabel("APPROVED")).toBe("已通过");
    expect(productionReviewStatusLabel("REGENERATE")).toBe("待返工");
    expect(productionStatusLabel("READY")).not.toBe(productionStatusLabel("RUNNING"));
    expect(comfyStatusLabel("CONNECTED")).toBe("已连接");
    expect(comfyStatusLabel("OFFLINE")).toBe("离线");
    expect(taskStatusLabel("UNKNOWN_STATUS")).toBe("未知状态");
  });

  it("localizes asset, workflow, project, and mode display labels", () => {
    expect(assetCategoryLabel("generated_image")).toBe("生成图片");
    expect(assetTypeLabel({ assetType: "video", category: "generated_video" })).toBe("生成视频");
    expect(assetDisplayName({ category: "generated_image", name: "Generated Image 1" })).toBe("生成图片 1");
    expect(assetDisplayName({ category: "source_image", name: "My Reference.png" })).toBe("My Reference.png");
    expect(workflowModeLabel("reference_to_video")).toBe("参考素材生成视频");
    expect(workflowDisplayName("wfl_kera2_t2i_local_v2", "原始工作流名")).toBe("Krea2 文生图");
    expect(workflowDisplayName(undefined, "Krea2 T2I Local")).toBe("Krea2 文生图");
    expect(workflowDisplayName("custom_workflow", "用户工作流")).toBe("用户工作流");
    expect(projectDisplayName("prj_default", "系统默认项目")).toBe("默认项目");
    expect(projectDisplayName("prj_custom", "我的项目")).toBe("我的项目");
  });

  it("keeps date formatting Chinese and handles invalid values safely", () => {
    expect(formatDateTime("not-a-date")).toBe("时间未知");
    expect(formatDateTime("2026-01-02T03:04:05.000Z")).toMatch(/2026/);
  });
});

describe("用户可见错误信息", () => {
  it("maps known backend error codes to Chinese", () => {
    expect(toUserMessage({ code: "COMFY_OFFLINE", message: "connection refused" })).toContain("ComfyUI");
    expect(errorMessageForCode("TASK_NOT_CANCELLABLE")).toBe("当前任务状态不支持取消。");
  });

  it("keeps runtime admission identity in the user-visible start error", () => {
    const formatted = formatUiError({
      code: "PRODUCTION_START_ADMISSION_BLOCKED",
      message: "Production batch runtime admission failed",
      details: {
        code: "RUNTIME_ADMISSION_MISSING_NODES",
        workflowVersionId: "wfv_h3",
        recipeId: "rcp_h3",
        reason: "ComfyUI is missing workflow node classes",
        missingNodes: ["KSampler", "VAEEncode"],
      },
    });

    expect(formatted.code).toBe("PRODUCTION_START_ADMISSION_BLOCKED");
    expect(formatted.message).toContain("wfv_h3");
    expect(formatted.message).toContain("rcp_h3");
    expect(formatted.message).toContain("KSampler");
    expect(toUserMessage({
      code: "PRODUCTION_START_ADMISSION_BLOCKED",
      message: "RUNTIME_ADMISSION_MISSING_NODES: workflow_version_id=wfv_h3, recipe_id=rcp_h3, reason=ComfyUI is missing workflow node classes, missing_nodes=KSampler",
    })).toContain("缺少节点");
  });

  it("prefers a structured IPC code over a code embedded in the message", () => {
    const formatted = formatUiError({
      code: "INVALID_INPUT",
      message: "RUNTIME_ADMISSION_MISSING_NODES: legacy technical text",
    });

    expect(formatted.code).toBe("INVALID_INPUT");
    expect(formatted.message).toBe("输入内容无效，请检查后重试。");
  });

  it("uses the structured workflow recipe lifecycle code without parsing the message", () => {
    const formatted = formatUiError({
      code: "WORKFLOW_RECIPE_LIFECYCLE_ERROR",
      message: "WORKFLOW_RECIPE_LAST_ACTIVE_GUARD: technical text",
      details: {
        workflowRecipeErrorCode: "WORKFLOW_RECIPE_LAST_ACTIVE_GUARD",
        workflowVersionId: "wfv-1",
        recipeId: "recipe-1",
      },
    });

    expect(formatted.code).toBe("WORKFLOW_RECIPE_LAST_ACTIVE_GUARD");
    expect(formatted.message).toContain("最后一个可用配方");
  });

  it("does not expose unknown raw errors in the primary message", () => {
    const formatted = formatUiError(new Error("SECRET_RAW_ERROR from backend"));
    expect(formatted.message).toBe("操作失败，请查看技术详情。");
    expect(formatted.message).not.toContain("SECRET_RAW_ERROR");
    expect(formatted.technicalMessage).toContain("SECRET_RAW_ERROR");
  });

});
