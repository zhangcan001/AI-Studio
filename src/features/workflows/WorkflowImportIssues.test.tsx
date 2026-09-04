// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkflowImportIssues, workflowIssueSelectionKey } from "./WorkflowImportIssues";
import type {
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowOnboardingDraftView,
} from "../../types/workflowOnboarding";

afterEach(() => cleanup());

const issue1: WorkflowAutoIssueView = {
  code: "AMBIGUOUS_INPUT",
  field: "prompt",
  message: "提示词需要确认",
  candidates: [{ label: "A1" }, { label: "A2" }],
};

const issue2: WorkflowAutoIssueView = {
  code: "AMBIGUOUS_INPUT",
  field: "reference_image",
  message: "参考图需要确认",
  candidates: [{ label: "B1" }, { label: "B2" }],
};

function planWithIssues(
  issues: WorkflowAutoIssueView[],
  overrides: Partial<WorkflowAutoOnboardingPlanView> = {},
): WorkflowAutoOnboardingPlanView {
  return {
    draftId: "draft-1",
    state: "NEEDS_REVIEW",
    workflowKind: "IMAGE",
    workflowSha256: "sha-1",
    originalFilename: "demo.json",
    nodeCount: 2,
    uniqueClassCount: 2,
    metadata: {
      workflowId: "workflow-1",
      name: "Demo Workflow",
      workflowVersion: "1.0.0",
      recipeVersion: "1.0.0",
      category: "image",
      mode: "IMAGE",
      recipeId: "recipe-1",
    },
    capability: { state: "READY", issues: [] },
    inputMappings: [],
    outputMappings: [],
    validation: {
      apiFormat: true,
      recipe: false,
      bindings: false,
      outputs: true,
      manifest: true,
      capability: true,
      dryRun: false,
      readyToPublish: false,
      issues: [],
    },
    inferences: [],
    issues,
    autoPublishable: false,
    message: "needs review",
    ...overrides,
  };
}

function draftWithOptions(plan: WorkflowAutoOnboardingPlanView): WorkflowOnboardingDraftView {
  return {
    draftId: plan.draftId,
    workflowSha256: plan.workflowSha256,
    originalFilename: plan.originalFilename,
    nodeCount: 1,
    uniqueClassCount: 1,
    nodes: [{
      nodeId: "7",
      classType: "CheckpointLoader",
      title: "Checkpoint Loader",
      isOutputNode: false,
      inputs: [{
        name: "ckpt_name",
        kind: "literal",
        currentValueSummary: "missing.safetensors",
        isLinked: false,
        bindable: true,
        suggestedType: "textarea",
        suggestedSemanticKey: "model",
        numericMin: undefined,
        numericMax: undefined,
        numericStep: undefined,
        allowedOptions: ["available.safetensors", "other.safetensors"],
      }],
    }],
    capability: plan.capability,
    inputMappings: plan.inputMappings,
    outputMappings: plan.outputMappings,
    manifest: plan.metadata,
    recipe: { inputs: [], bindings: [], outputs: plan.outputMappings, valid: false, issues: [] },
    validation: plan.validation,
  };
}

describe("WorkflowImportIssues", () => {
  it("隔离相同 code 的不同 field，并向 resolve 传递准确 issue 与 candidate", async () => {
    const user = userEvent.setup();
    const onResolve = vi.fn();

    expect(workflowIssueSelectionKey(issue1, 0)).toBe("AMBIGUOUS_INPUT:prompt:0");
    expect(workflowIssueSelectionKey(issue2, 1)).toBe("AMBIGUOUS_INPUT:reference_image:1");

    render(
      <WorkflowImportIssues
        plan={planWithIssues([issue1, issue2])}
        loading={false}
        onResolve={onResolve}
        onResume={vi.fn()}
        onOpenAdvanced={vi.fn()}
        onOpenExisting={vi.fn()}
      />,
    );

    const promptA2 = screen.getByRole("radio", { name: "A2" }) as HTMLInputElement;
    const referenceB1 = screen.getByRole("radio", { name: "B1" }) as HTMLInputElement;
    await user.click(promptA2);
    expect(promptA2.checked).toBe(true);
    expect(referenceB1.checked).toBe(false);

    const resolveButtons = screen.getAllByRole("button", { name: "确认这项并继续" });
    await user.click(resolveButtons[0]);
    expect(onResolve).toHaveBeenNthCalledWith(1, issue1, issue1.candidates[1]);

    await user.click(referenceB1);
    expect(promptA2.checked).toBe(true);
    expect(referenceB1.checked).toBe(true);
    await user.click(resolveButtons[1]);
    expect(onResolve).toHaveBeenNthCalledWith(2, issue2, issue2.candidates[0]);
  });

  it("把图推断、输出、缺失节点和不可用输入选项展开为可定位信息", () => {
    const issues: WorkflowAutoIssueView[] = [
      {
        code: "AMBIGUOUS_DURATION_SOURCE",
        field: "duration_seconds",
        message: "无法自动确认视频时长来源",
        candidates: [
          { label: "节点 49 · value", nodeId: "49", inputName: "value", fieldType: "number" },
          { label: "节点 51 · value", nodeId: "51", inputName: "value", fieldType: "number" },
        ],
      },
      {
        code: "AMBIGUOUS_OUTPUT",
        field: "output_1",
        message: "检测到多个视频输出节点",
        candidates: [
          { label: "节点 62 · VHS_VideoCombine", nodeId: "62", outputId: "output_1", outputType: "video" },
          { label: "节点 70 · SaveVideo", nodeId: "70", outputId: "output_1", outputType: "video" },
        ],
      },
      {
        code: "MISSING_NODES",
        message: "ComfyUI 缺少工作流节点",
        candidates: [],
      },
      {
        code: "INPUT_OPTION_UNAVAILABLE",
        field: "ckpt_name",
        message: "当前 ComfyUI 中缺少工作流所需的模型或选项",
        candidates: [],
      },
    ];
    const plan = planWithIssues(issues, {
      capability: {
        state: "MISSING_NODES",
        issues: [
          {
            code: "MISSING_NODE",
            classType: "ComfyMathExpression",
            affectedNodeIds: ["35"],
            message: "Missing ComfyUI node class ComfyMathExpression",
          },
          {
            code: "INPUT_OPTION_UNAVAILABLE",
            classType: "CheckpointLoader",
            nodeId: "7",
            affectedNodeIds: [],
            inputName: "ckpt_name",
            currentValue: "missing.safetensors",
            message: "Current ComfyUI does not offer this workflow value.",
          },
        ],
      },
    });

    render(
      <WorkflowImportIssues
        plan={plan}
        draft={draftWithOptions(plan)}
        loading={false}
        onResolve={vi.fn()}
        onResume={vi.fn()}
        onOpenAdvanced={vi.fn()}
        onOpenExisting={vi.fn()}
      />,
    );

    expect(screen.getByText("无法自动确认视频时长来源", { selector: "strong" })).toBeTruthy();
    expect(screen.getByText("字段：duration_seconds")).toBeTruthy();
    expect(screen.getByText("节点 49 · value")).toBeTruthy();
    expect(screen.getByText("检测到多个输出节点", { selector: "strong" })).toBeTruthy();
    expect(screen.getByText("节点 62 · VHS_VideoCombine")).toBeTruthy();
    expect(screen.getByText(/节点.*35/)).toBeTruthy();
    expect(screen.getByText(/输入.*ckpt_name/)).toBeTruthy();
    expect(screen.getByText(/候选.*available\.safetensors/)).toBeTruthy();
  });

  it("Recipe 过期时保留现有身份、语义匹配诊断和重新生成入口", async () => {
    const user = userEvent.setup();
    const onRegenerateRecipe = vi.fn();
    const outdatedIssue: WorkflowAutoIssueView = {
      code: "EXISTING_RECIPE_OUTDATED",
      message: "existing recipe is outdated",
      candidates: [],
    };
    const outdatedPlan = planWithIssues([outdatedIssue], {
      state: "WAITING_FOR_COMFY_UI",
      workflowKind: "VIDEO",
      metadata: {
        ...planWithIssues([]).metadata,
        name: "导入文件名",
        mode: "T2V",
      },
      capability: { state: "COMFY_OFFLINE", issues: [] },
      existingWorkflowId: "builtin-workflow",
      existingWorkflowVersion: "1.0.0",
      existingWorkflowName: "AITUDOU MiniMax H3 LightX2V 8步高动态加速",
      existingWorkflowSource: "BUILTIN",
      existingPackageName: "builtin-package",
      existingMatchType: "SEMANTIC_SHA",
      existingRecipes: [{ recipeId: "rcp-old", recipeVersion: "1.0.0", packageName: "builtin-package" }],
      suggestedRecipeVersion: "1.0.1",
    });

    const { rerender } = render(
      <WorkflowImportIssues
        plan={outdatedPlan}
        loading={false}
        onResolve={vi.fn()}
        onResume={vi.fn()}
        onOpenAdvanced={vi.fn()}
        onOpenExisting={vi.fn()}
        onRegenerateRecipe={onRegenerateRecipe}
      />,
    );

    expect(screen.getByRole("heading", { name: "检测到现有工作流，需要更新配置" })).toBeTruthy();
    expect(screen.getByText("AITUDOU MiniMax H3 LightX2V 8步高动态加速")).toBeTruthy();
    expect(screen.getByText("系统自带")).toBeTruthy();
    expect(screen.getByText("1.0.0 · rcp-old")).toBeTruthy();
    expect(screen.getByText("1.0.1")).toBeTruthy();
    expect(screen.getByText("matchType=语义匹配")).toBeTruthy();
    expect(screen.getByText(/连接 ComfyUI 后可重新生成 Recipe/)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "工作流暂时无法添加" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "更新工作流配置" }));
    expect(onRegenerateRecipe).toHaveBeenCalledTimes(1);

    rerender(
      <WorkflowImportIssues
        plan={planWithIssues([outdatedIssue], {
          ...outdatedPlan,
          state: "BLOCKED",
          capability: { state: "MISSING_NODES", issues: [] },
        })}
        loading={false}
        onResolve={vi.fn()}
        onResume={vi.fn()}
        onOpenAdvanced={vi.fn()}
        onOpenExisting={vi.fn()}
        onRegenerateRecipe={onRegenerateRecipe}
      />,
    );
    expect(screen.getByText(/修复节点或输入后可重新生成 Recipe/)).toBeTruthy();
    expect(screen.getByText("matchType=语义匹配")).toBeTruthy();
  });

  it("结构相似工作流要求用户明确选择新工作流或新版本", async () => {
    const user = userEvent.setup();
    const onOpenExistingVersion = vi.fn();
    render(
      <WorkflowImportIssues
        plan={planWithIssues([], {
          state: "NEEDS_REVIEW",
          recognition: { format: "API", identity: "STRUCTURAL_VARIANT" },
          identity: "STRUCTURAL_VARIANT",
          existingWorkflowId: "workflow-existing",
          existingWorkflowVersion: "1.0.0",
          existingWorkflowName: "现有工作流",
          existingMatchType: "STRUCTURAL_SHA",
        })}
        loading={false}
        onResolve={vi.fn()}
        onResume={vi.fn()}
        onOpenAdvanced={vi.fn()}
        onOpenExisting={vi.fn()}
        onOpenExistingVersion={onOpenExistingVersion}
      />,
    );

    expect(screen.getByRole("heading", { name: "检测到结构相似的工作流" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "添加为现有工作流的新版本" }));
    expect(onOpenExistingVersion).toHaveBeenCalledTimes(1);
  });
});
