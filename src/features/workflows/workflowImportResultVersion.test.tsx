import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { WorkflowImportResult } from "./WorkflowImportResult";
import type { WorkflowAutoOnboardingPlanView } from "../../types/workflowOnboarding";

function plan(publishedVersion: string): WorkflowAutoOnboardingPlanView {
  return {
    draftId: "onb_1",
    workflowKind: "IMAGE",
    metadata: { name: "W19", category: "image", mode: "text_to_image", workflowVersion: "1.0.1" },
    inputMappings: [],
    outputMappings: [],
    capability: { state: "READY", issues: [] },
    semanticCapabilityStatus: "READY",
    runtimeImportStatus: "READY",
    runtimeImportBlockers: [],
    published: {
      workflowId: "wfl_1",
      workflowVersion: publishedVersion,
      recipeId: "rcp_1",
      packageName: "pkg",
      workflowSha256: "sha",
      workflowVersionId: "wv_1",
    },
  } as unknown as WorkflowAutoOnboardingPlanView;
}

describe("W-19 提交时分配版本", () => {
  it("shows the version decided at commit instead of the provisional draft version", () => {
    const markup = renderToStaticMarkup(<WorkflowImportResult plan={plan("1.0.2")} onOpenAdvanced={vi.fn()} />);
    expect(markup).toContain("工作流版本<strong>1.0.2</strong>");
    expect(markup).not.toContain("工作流版本<strong>1.0.1</strong>");
  });
});
