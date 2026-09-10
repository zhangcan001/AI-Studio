import { describe, expect, it } from "vitest";
import type { WorkflowInputView } from "../../types/workflowOnboarding";
import {
  createDefaultOutputDraft,
  isExposableWorkflowInput,
  latestCatalogRecipeForWorkflowItem,
  normalizeWorkspaceItem,
  resolveImplicitWorkflowRecipe,
} from "./WorkflowWorkspace";
import { resolveProjectWorkflow } from "../runtime/projectWorkflowResolution";

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

const workflowRecipeItem = (recipes: Array<[string, string]>) =>
  ({
    workflowVersionId: "WV1",
    recipes: recipes.map(([recipeId, version]) => ({ recipeId, version })),
  }) as Parameters<typeof latestCatalogRecipeForWorkflowItem>[0];

const catalogRecipes = (...entries: Array<[string, string, string]>) =>
  entries.map(([workflowVersionId, recipeId, recipeVersion]) => ({ workflowVersionId, recipeId, recipeVersion })) as Parameters<
    typeof latestCatalogRecipeForWorkflowItem
  >[1];

describe("工作流最新目录配方匹配", () => {
  it("返回项目中最新且已进入目录的配方", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem([["R1", "1.0.0"], ["R2", "1.0.1"]]),
        catalogRecipes(["WV1", "R1", "1.0.0"], ["WV1", "R2", "1.0.1"]),
      )?.recipeId,
    ).toBe("R2");
  });

  it("最新配方不在目录时回退到更早的匹配配方", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem([["R1", "1.0.0"], ["R2", "1.0.1"]]),
        catalogRecipes(["WV1", "R1", "1.0.0"]),
      )?.recipeId,
    ).toBe("R1");
  });

  it("相同配方 ID 但不同工作流版本时不匹配", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem([["R2", "1.0.1"]]),
        catalogRecipes(["WV2", "R2", "1.0.1"]),
      ),
    ).toBeUndefined();
  });

  it("不依赖工作区或目录插入顺序，按有效 semver 选择最新配方", () => {
    expect(
      latestCatalogRecipeForWorkflowItem(
        workflowRecipeItem([["R10", "1.0.10"], ["R2", "1.0.2"]]),
        catalogRecipes(["WV1", "R2", "1.0.2"], ["WV1", "R10", "1.0.10"]),
      )?.recipeId,
    ).toBe("R10");
  });
});

type ImplicitRecipeItem = Parameters<typeof resolveImplicitWorkflowRecipe>[0];

const implicitRecipeItem = (overrides: Partial<ImplicitRecipeItem> = {}) => ({
  workflowVersionId: "WV1",
  recipes: [
    { recipeId: "R_LEGACY", version: "1.0.0", inputCount: 0, outputCount: 0 },
    { recipeId: "R_PROMOTED", version: "2.0.0", inputCount: 0, outputCount: 0 },
  ],
  currentRecipe: undefined,
  archived: false,
  libraryState: "ACTIVE",
  ...overrides,
}) as ImplicitRecipeItem;

describe("DEV-091 推广配方消费策略", () => {
  const recipeSummary = (recipeId: string, version: string) => ({ recipeId, version, inputCount: 0, outputCount: 0 });

  it("显式 workflowVersionId + recipeId 优先于推广配方", () => {
    const explicit = catalogRecipes(["WV1", "R_EXPLICIT", "1.0.0"])[0];
    const promoted = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    expect(resolveProjectWorkflow({
      candidates: [explicit, promoted],
      explicit: { workflowVersionId: "WV1", recipeId: "R_EXPLICIT" },
      recommended: promoted,
    }).recipe).toBe(explicit);
  });

  it("项目精确绑定优先于推广配方", () => {
    const project = catalogRecipes(["WV1", "R_PROJECT", "1.0.0"])[0];
    const promoted = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    expect(resolveProjectWorkflow({
      candidates: [project, promoted],
      projectDefault: { workflowVersionId: "WV1", recipeId: "R_PROJECT" },
      recommended: promoted,
    }).recipe).toBe(project);
  });

  it("历史精确配方引用保持不变", () => {
    const historical = catalogRecipes(["WV1", "R_HISTORY", "1.0.0"])[0];
    const promoted = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    expect(resolveProjectWorkflow({
      candidates: [historical, promoted],
      explicit: { workflowVersionId: "WV1", recipeId: "R_HISTORY" },
      recommended: promoted,
    }).recipe?.recipeId).toBe("R_HISTORY");
  });

  it("已确定版本且没有显式配方时优先使用该版本的推广配方", () => {
    const promoted = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem({
        currentRecipe: { workflowVersionId: "WV1", recipeId: "R_PROMOTED", isPromoted: true },
      }),
      [catalogRecipes(["WV1", "R_LEGACY", "1.0.0"])[0], promoted],
    )).toBe(promoted);
  });

  it("没有推广配方时保持原有最新配方回退", () => {
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem(),
      catalogRecipes(["WV1", "R_LEGACY", "1.0.0"], ["WV1", "R_PROMOTED", "2.0.0"]),
    )?.recipeId).toBe("R_PROMOTED");
  });

  it("推广配方不会跨工作流版本选择", () => {
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem({ currentRecipe: { workflowVersionId: "WV2", recipeId: "R_PROMOTED", isPromoted: true } }),
      catalogRecipes(["WV1", "R_LEGACY", "1.0.0"], ["WV2", "R_PROMOTED", "9.0.0"]),
    )?.recipeId).toBe("R_LEGACY");
  });

  it("推广元数据不会改变 Registry 已确定的当前版本 authority", () => {
    const currentVersion = {
      workflowVersionId: "WV3",
      recipes: [{ recipeId: "R_CURRENT", version: "3.0.0" }],
    };
    const item = normalizeWorkspaceItem({
      registry: {
        workflowId: "WF1",
        name: "Workflow",
        sourceKind: "USER",
        libraryState: "ACTIVE",
        currentVersionId: "WV3",
        currentVersion,
        currentRecipe: { workflowVersionId: "WV3", recipeId: "R_CURRENT" },
        versions: [
          { workflowVersionId: "WV2", recipes: [{ workflowVersionId: "WV2", recipeId: "R_OLD", isPromoted: true }] },
          currentVersion,
        ],
        recipes: [{ workflowVersionId: "WV2", recipeId: "R_OLD", isPromoted: true }, { workflowVersionId: "WV3", recipeId: "R_CURRENT" }],
        projectUsageCount: 0,
        historyCount: 0,
      },
      runtime: [],
    });
    expect(item.workflowVersionId).toBe("WV3");
    expect(item.currentRecipe?.recipeId).toBe("R_CURRENT");
  });

  it("归档工作流不会因推广标记重新变得可用", () => {
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem({ archived: true, currentRecipe: { workflowVersionId: "WV1", recipeId: "R_PROMOTED", isPromoted: true } }),
      catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"]),
    )).toBeUndefined();
  });

  it("推广配方缺失时安全回退且不崩溃", () => {
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem({ currentRecipe: { workflowVersionId: "WV1", recipeId: "R_MISSING", isPromoted: true } }),
      catalogRecipes(["WV1", "R_LEGACY", "1.0.0"]),
    )?.recipeId).toBe("R_LEGACY");
  });

  it("只按 recipeId 精确匹配，不按配方名称猜测", () => {
    const sameName = { ...catalogRecipes(["WV1", "R_NAME_MATCH", "1.0.0"])[0], name: "Promoted" };
    const promoted = { ...catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0], name: "Promoted" };
    expect(resolveImplicitWorkflowRecipe(
      implicitRecipeItem({
        recipes: [recipeSummary("R_NAME_MATCH", "1.0.0")],
        currentRecipe: { workflowVersionId: "WV1", recipeId: "R_PROMOTED", isPromoted: true },
      }),
      [sameName, promoted],
    )?.recipeId).toBe("R_NAME_MATCH");
  });

  it("为 GenerationStudio 提供已解析的精确目录配方，而不在其内建立第二套规则", () => {
    const authoritative = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    const resolved = resolveImplicitWorkflowRecipe(
      implicitRecipeItem({ currentRecipe: { workflowVersionId: "WV1", recipeId: "R_PROMOTED", isPromoted: true } }),
      [authoritative],
    );
    expect(resolved).toBe(authoritative);
    expect(resolved).toMatchObject({ workflowVersionId: "WV1", recipeId: "R_PROMOTED" });
  });

  it("批次、任务、预设和实验的精确 identity 不被隐式推广改变", () => {
    const exact = catalogRecipes(["WV1", "R_EXACT", "1.0.0"])[0];
    const promoted = catalogRecipes(["WV1", "R_PROMOTED", "2.0.0"])[0];
    expect(resolveProjectWorkflow({
      candidates: [exact, promoted],
      explicit: { workflowVersionId: exact.workflowVersionId, recipeId: exact.recipeId },
      recommended: promoted,
    }).recipe).toMatchObject({ workflowVersionId: "WV1", recipeId: "R_EXACT" });
  });

  it("更换推广配方只影响后续的隐式解析", () => {
    const catalog = catalogRecipes(["WV1", "R_A", "1.0.0"], ["WV1", "R_B", "2.0.0"]);
    const recipes = [recipeSummary("R_A", "1.0.0"), recipeSummary("R_B", "2.0.0")];
    const first = resolveImplicitWorkflowRecipe(implicitRecipeItem({ recipes, currentRecipe: { workflowVersionId: "WV1", recipeId: "R_A", isPromoted: true } }), catalog);
    const second = resolveImplicitWorkflowRecipe(implicitRecipeItem({ recipes, currentRecipe: { workflowVersionId: "WV1", recipeId: "R_B", isPromoted: true } }), catalog);
    expect(first?.recipeId).toBe("R_A");
    expect(second?.recipeId).toBe("R_B");
  });

  it("切换项目时不会泄漏上一个项目的推广推导状态", () => {
    const catalog = catalogRecipes(["WV1", "R_A", "1.0.0"], ["WV2", "R_B", "1.0.0"]);
    const projectA = resolveImplicitWorkflowRecipe(implicitRecipeItem({ workflowVersionId: "WV1", recipes: [recipeSummary("R_A", "1.0.0")], currentRecipe: { workflowVersionId: "WV1", recipeId: "R_A", isPromoted: true } }), catalog);
    const projectB = resolveImplicitWorkflowRecipe(implicitRecipeItem({ workflowVersionId: "WV2", recipes: [recipeSummary("R_B", "1.0.0")] }), catalog);
    expect(projectA?.recipeId).toBe("R_A");
    expect(projectB?.recipeId).toBe("R_B");
  });
});
