import { describe, expect, it } from "vitest";
import type { WorkflowInputView } from "../../types/workflowOnboarding";
import { createDefaultOutputDraft, isExposableWorkflowInput } from "./WorkflowWorkspace";

describe("工作流输出映射默认值", () => {
  it("使用中文显示名称，同时保留技术输出 ID", () => {
    const draft = createDefaultOutputDraft();

    expect(draft.outputId).toBe("output_1");
    expect(draft.label).toBe("输出结果");
    expect(draft.label).not.toBe("Output");
  });
});

function workflowInput(overrides: Partial<WorkflowInputView> = {}): WorkflowInputView {
  return {
    name: "steps",
    kind: "literal",
    currentValueSummary: "20",
    isLinked: false,
    bindable: true,
    suggestedType: "integer",
    suggestedSemanticKey: "steps",
    numericMin: "1",
    numericMax: "100",
    numericStep: "1",
    allowedOptions: [],
    ...overrides,
  };
}

describe("Workflow Parameter Exposure 安全边界", () => {
  it("只允许已有 Recipe 字段类型支持的未连接字面量输入", () => {
    expect(isExposableWorkflowInput(workflowInput())).toBe(true);
    expect(isExposableWorkflowInput(workflowInput({ isLinked: true }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ bindable: false }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ suggestedType: "float" }))).toBe(false);
  });

  it("模型、路径和设备类输入保持 Workflow 内部状态", () => {
    expect(isExposableWorkflowInput(workflowInput({ name: "model", suggestedSemanticKey: "model" }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ name: "output_directory", suggestedSemanticKey: "output_directory" }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ name: "device", suggestedSemanticKey: "device" }))).toBe(false);
  });
});
