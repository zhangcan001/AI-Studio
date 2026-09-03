// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkflowImportFormatIssue } from "./WorkflowImportIssues";
import { WorkflowSmartImport, workflowImportFormat } from "./WorkflowSmartImport";
import type { WorkflowAutoOnboardingPlanView } from "../../types/workflowOnboarding";

afterEach(() => cleanup());

function plan(overrides: Partial<WorkflowAutoOnboardingPlanView> = {}): WorkflowAutoOnboardingPlanView {
  return {
    draftId: "draft-1",
    state: "AUTO_PUBLISHED",
    workflowKind: "VIDEO",
    workflowSha256: "sha-1",
    originalFilename: "demo.json",
    nodeCount: 4,
    uniqueClassCount: 3,
    metadata: {
      workflowId: "workflow-1",
      name: "Demo Workflow",
      workflowVersion: "1.0.0",
      recipeVersion: "1.0.0",
      category: "video",
      mode: "CUSTOM_VIDEO",
      recipeId: "recipe-1",
    },
    capability: { state: "READY", issues: [] },
    inputMappings: [{
      semanticKey: "prompt",
      fieldType: "textarea",
      label: "提示词",
      required: true,
      targetNode: "1",
      targetInput: "text",
    }],
    outputMappings: [{ outputId: "output_1", label: "视频", type: "video", nodeId: "4", required: true }],
    validation: {
      apiFormat: true,
      recipe: true,
      bindings: true,
      outputs: true,
      manifest: true,
      capability: true,
      dryRun: true,
      readyToPublish: true,
      issues: [],
    },
    inferences: [],
    issues: [],
    autoPublishable: true,
    published: {
      workflowId: "workflow-1",
      workflowVersion: "1.0.0",
      recipeId: "recipe-1",
      packageName: "Demo Workflow",
      workflowSha256: "sha-1",
      refreshed: { packagesFound: 1, valid: 1, invalid: 0, inserted: 1, reused: 0, errors: [] },
    },
    message: "published",
    ...overrides,
  };
}

function smartImportProps() {
  return {
    loading: false,
    onResolve: vi.fn(),
    onResume: vi.fn(),
    onOpenAdvanced: vi.fn(),
    onOpenExisting: vi.fn(),
  };
}

describe("DEV-079 添加工作流前端 UAT", () => {
  it("API 自动添加成功后显示用户结果和三类后续动作", async () => {
    const user = userEvent.setup();
    const useInProject = vi.fn();
    const openStudio = vi.fn();
    const returnToList = vi.fn();

    render(
      <WorkflowSmartImport
        plan={plan()}
        projectId="project-1"
        {...smartImportProps()}
        onUseInProject={useInProject}
        onOpenStudio={openStudio}
        onReturnToList={returnToList}
      />,
    );

    expect(screen.getByRole("heading", { name: "✓ 工作流已添加" })).toBeTruthy();
    expect(screen.getByText("Demo Workflow")).toBeTruthy();
    expect(screen.getByText("用途")).toBeTruthy();
    expect(screen.getByRole("button", { name: "用于当前项目" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "打开生成页面" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "用于当前项目" }));
    await user.click(screen.getByRole("button", { name: "打开生成页面" }));
    await user.click(screen.getByRole("button", { name: "返回工作流列表" }));
    expect(useInProject).toHaveBeenCalledWith("workflow-1", "recipe-1");
    expect(openStudio).toHaveBeenCalledWith("workflow-1", "recipe-1");
    expect(returnToList).toHaveBeenCalledTimes(1);
  });

  it("识别 UI JSON 时只显示导出指引，不进入发布成功态", async () => {
    const user = userEvent.setup();
    const props = smartImportProps();
    const retry = vi.fn();

    expect(workflowImportFormat(plan({ format: "UI", state: "BLOCKED" }))).toBe("UI");
    render(
      <WorkflowSmartImport
        plan={plan({ format: "UI", state: "BLOCKED" })}
        {...props}
        onRetry={retry}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByText("检测到 ComfyUI 普通工作流 JSON")).toBeTruthy();
    expect(screen.getByText("请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。")).toBeTruthy();
    expect(screen.queryByText("✓ 工作流已添加")).toBeNull();
    await user.click(screen.getByRole("button", { name: "选择另一个文件" }));
    expect(retry).toHaveBeenCalledTimes(1);
  });

  it("非法 JSON 和未知 JSON 都停留在未添加态", () => {
    const onRetry = vi.fn();
    const onCancel = vi.fn();
    const { rerender } = render(
      <WorkflowImportFormatIssue
        issue={{ kind: "INVALID_JSON", message: "无法读取这个文件，它不是有效的 JSON。" }}
        loading={false}
        onRetry={onRetry}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByText("无法读取这个文件，它不是有效的 JSON。")).toBeTruthy();

    rerender(
      <WorkflowImportFormatIssue
        issue={{ kind: "UNKNOWN_FORMAT", message: "这个 JSON 不是可识别的 ComfyUI 工作流。" }}
        loading={false}
        onRetry={onRetry}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByText("这个 JSON 不是可识别的 ComfyUI 工作流。")).toBeTruthy();
    expect(screen.queryByText("✓ 工作流已添加")).toBeNull();
  });
});
