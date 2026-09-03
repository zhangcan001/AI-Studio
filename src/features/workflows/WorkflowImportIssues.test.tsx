// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkflowImportIssues, workflowIssueSelectionKey } from "./WorkflowImportIssues";
import type { WorkflowAutoIssueView, WorkflowAutoOnboardingPlanView } from "../../types/workflowOnboarding";

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

function planWithIssues(issues: WorkflowAutoIssueView[]): WorkflowAutoOnboardingPlanView {
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
});
