import { describe, expect, it } from "vitest";
import {
  assetCategoryLabel,
  assetDisplayName,
  assetTypeLabel,
  comfyStatusLabel,
  formatDateTime,
  projectDisplayName,
  productionItemStatusLabel,
  productionStatusLabel,
  taskStatusLabel,
  workflowDisplayName,
  workflowModeLabel,
} from "./statusLabels";
import { errorMessageForCode, formatUiError, toUserMessage } from "./errorMessages";

describe("简体中文状态展示", () => {
  it("maps task and production status values without changing protocol values", () => {
    expect(taskStatusLabel("RUNNING")).toBe("生成中");
    expect(taskStatusLabel("CANCEL_REQUESTED")).toBe("正在取消");
    expect(productionStatusLabel("PAUSED")).toBe("已暂停");
    expect(productionItemStatusLabel("DISPATCHED")).toBe("执行中");
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
    expect(workflowDisplayName("wfl_kera2_t2i_local_v2", "原始工作流名")).toBe("Kera2 文生图");
    expect(workflowDisplayName(undefined, "Krea2 T2I Local")).toBe("Kera2 文生图");
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

  it("does not expose unknown raw errors in the primary message", () => {
    const formatted = formatUiError(new Error("SECRET_RAW_ERROR from backend"));
    expect(formatted.message).toBe("操作失败，请查看技术详情。");
    expect(formatted.message).not.toContain("SECRET_RAW_ERROR");
    expect(formatted.technicalMessage).toContain("SECRET_RAW_ERROR");
  });
});
