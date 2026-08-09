import { describe, expect, it } from "vitest";
import { createDefaultOutputDraft } from "../features/workflows/WorkflowWorkspace";

const forbiddenExactCopy = [
  "Studio",
  "Assets",
  "Tasks",
  "Projects",
  "Workflows",
  "Generate",
  "Cancel",
  "Save Preset",
  "Delete Preset",
  "Output",
  "Preview",
  "Loading...",
];

const forbiddenPhrases = [
  "New Project",
  "Open Queue",
  "Production queue active",
  "Task recovery",
  "Generated Image",
  "Generated Video",
];

const productionUiCopyFixture = [
  "AI Studio",
  "创作",
  "资产库",
  "任务",
  "项目",
  "工作流",
  "ComfyUI 状态",
  "接口地址",
  "版本",
  "GPU",
  "VRAM",
  "节点数量",
  "导入 API 工作流",
  "输出映射",
  "显示名称",
  createDefaultOutputDraft().label,
  "图片输出",
  "视频输出",
  "预设",
  "保存预设",
  "删除预设",
  "生产队列",
  "任务恢复",
  "资产预览",
  "生成",
  "取消",
  "加载中……",
];

describe("正式页面中文文案守卫", () => {
  it("不重新出现已知的普通英文界面文案", () => {
    const findings = productionUiCopyFixture.filter(
      (value) =>
        forbiddenExactCopy.includes(value) || forbiddenPhrases.some((phrase) => value.includes(phrase)),
    );

    expect(findings).toEqual([]);
  });
});
