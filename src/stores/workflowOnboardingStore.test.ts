import { beforeEach, describe, expect, it } from "vitest";
import { useWorkflowOnboardingStore } from "./workflowOnboardingStore";

describe("workflow onboarding store", () => {
  beforeEach(() => useWorkflowOnboardingStore.getState().reset());

  it("keeps the draft wizard state separate from the Studio store", () => {
    const draft = {
      draftId: "onb_test",
      workflowSha256: "sha",
      originalFilename: "workflow.json",
      nodeCount: 1,
      uniqueClassCount: 1,
      nodes: [],
      capability: { state: "NOT_CHECKED" as const, issues: [] },
      inputMappings: [],
      outputMappings: [],
      manifest: {
        workflowId: "wfl_test",
        name: "Test",
        workflowVersion: "1.0.0",
        recipeVersion: "1.0.0",
        category: "image",
        mode: "text_to_image",
        recipeId: "rcp_test",
      },
      recipe: { inputs: [], bindings: [], outputs: [], valid: false, issues: [] },
      validation: {
        apiFormat: true,
        recipe: false,
        bindings: false,
        outputs: false,
        manifest: true,
        capability: false,
        dryRun: false,
        readyToPublish: false,
        issues: [],
      },
    };

    useWorkflowOnboardingStore.getState().setDraft(draft);
    useWorkflowOnboardingStore.getState().setStep("inputs");

    expect(useWorkflowOnboardingStore.getState().draft?.draftId).toBe("onb_test");
    expect(useWorkflowOnboardingStore.getState().step).toBe("inputs");
  });
});
