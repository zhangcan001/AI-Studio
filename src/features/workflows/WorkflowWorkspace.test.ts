import { describe, expect, it } from "vitest";
import type { WorkflowInputView } from "../../types/workflowOnboarding";
import {
  createDefaultOutputDraft,
  isExposableWorkflowInput,
  latestCatalogRecipeForWorkflowItem,
} from "./WorkflowWorkspace";

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
  it("只允许已有 Recipe 字段类型支持的字面量和安全图语义输入", () => {
    expect(isExposableWorkflowInput(workflowInput())).toBe(true);
    expect(isExposableWorkflowInput(workflowInput({ isLinked: true, bindable: false }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({
      name: "width",
      isLinked: true,
      bindable: false,
      suggestedSemanticKey: "width",
    }))).toBe(true);
    expect(isExposableWorkflowInput(workflowInput({ bindable: false }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ suggestedType: "float" }))).toBe(false);
  });

  it("模型、路径和设备类输入保持 Workflow 内部状态", () => {
    expect(isExposableWorkflowInput(workflowInput({ name: "model", suggestedSemanticKey: "model" }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ name: "output_directory", suggestedSemanticKey: "output_directory" }))).toBe(false);
    expect(isExposableWorkflowInput(workflowInput({ name: "device", suggestedSemanticKey: "device" }))).toBe(false);
  });
});

const workflowRecipeItem = (recipeIds: string[]) =>
  ({
    workflowVersionId: "WV1",
    recipes: recipeIds.map((recipeId) => ({ recipeId })),
  }) as Parameters<typeof latestCatalogRecipeForWorkflowItem>[0];

const catalogRecipes = (...entries: Array<[string, string]>) =>
  entries.map(([workflowVersionId, recipeId]) => ({ workflowVersionId, recipeId })) as Parameters<
    typeof latestCatalogRecipeForWorkflowItem
  >[1];

describe("工作流最新目录配方匹配", () => {
  it("返回项目中最新且已进入目录的配方", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem(["R1", "R2"]),
        catalogRecipes(["WV1", "R1"], ["WV1", "R2"]),
      )?.recipeId,
    ).toBe("R2");
  });

  it("最新配方不在目录时回退到更早的匹配配方", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem(["R1", "R2"]),
        catalogRecipes(["WV1", "R1"]),
      )?.recipeId,
    ).toBe("R1");
  });

  it("相同配方 ID 但不同工作流版本时不匹配", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem(["R2"]),
        catalogRecipes(["WV2", "R2"]),
      ),
    ).toBeUndefined();
  });
});
