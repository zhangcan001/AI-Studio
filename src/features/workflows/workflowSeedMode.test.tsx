import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SeedModeSelect } from "./WorkflowWorkspace";
import { mappingToDraft, seedModeRequest } from "./workflowParameterExposureModel";
import type { WorkflowInputMappingView } from "../../types/workflowOnboarding";

const seedMapping: WorkflowInputMappingView = {
  semanticKey: "seed",
  fieldType: "seed",
  label: "Seed",
  required: false,
  defaultValue: "123456",
  targetNode: "3",
  targetInput: "seed",
  seedMode: "fixed",
};

describe("W-22 种子模式", () => {
  it("carries the backend seed mode into the draft and request", () => {
    const draft = mappingToDraft(seedMapping);
    expect(draft.seedMode).toBe("fixed");
    expect(draft.defaultValue).toBe("123456");
    expect(seedModeRequest(draft)).toBe("fixed");
    expect(seedModeRequest({ ...draft, seedMode: "random" })).toBe("random");
  });

  it("keeps legacy mappings without a mode on the legacy path", () => {
    const draft = mappingToDraft({ ...seedMapping, seedMode: undefined });
    expect(draft.seedMode).toBe("");
    expect(seedModeRequest(draft)).toBeUndefined();
    expect(seedModeRequest({ ...draft, fieldType: "integer", seedMode: "fixed" })).toBeUndefined();
  });

  it("renders the selector with the fixed literal or random mode", () => {
    const fixed = renderToStaticMarkup(<SeedModeSelect value="fixed" defaultValue="42" onChange={vi.fn()} />);
    expect(fixed).toContain("种子模式");
    expect(fixed).toMatch(/<option value="fixed" selected="">/);
    const random = renderToStaticMarkup(<SeedModeSelect value="random" defaultValue="42" onChange={vi.fn()} />);
    expect(random).toMatch(/<option value="random" selected="">/);
    const legacy = renderToStaticMarkup(<SeedModeSelect value="" defaultValue="" onChange={vi.fn()} />);
    expect(legacy).toMatch(/<option value="random" selected="">/);
  });
});
