import { describe, expect, it } from "vitest";
import {
  analyzePromptTemplateText,
  customPromptVariableNames,
  extractPromptTemplateVariables,
  isPromptTemplateText,
  toggleOrderedPromptSelection,
} from "./promptTemplateState";

describe("Prompt Template frontend state", () => {
  it("detects templates and keeps variables deterministic", () => {
    const text = "{{ project.name }} / {{scene.name}} / {{scene.name}} / {{custom.camera}}";
    expect(isPromptTemplateText(text)).toBe(true);
    expect(extractPromptTemplateVariables(text)).toEqual(["project.name", "scene.name", "custom.camera"]);
    expect(analyzePromptTemplateText(text)).toMatchObject({
      isTemplate: true,
      builtinVariables: ["project.name", "scene.name"],
      customVariables: ["custom.camera"],
      requiresStructure: true,
    });
    expect(customPromptVariableNames(["custom.camera", "custom.mood"])).toEqual(["camera", "mood"]);
  });

  it("preserves anchor checkbox order and removes without re-sorting", () => {
    let selected = toggleOrderedPromptSelection([], "anchor-b", true);
    selected = toggleOrderedPromptSelection(selected, "anchor-a", true);
    expect(selected).toEqual(["anchor-b", "anchor-a"]);
    expect(toggleOrderedPromptSelection(selected, "anchor-b", false)).toEqual(["anchor-a"]);
  });

  it("treats an unfinished marker as a template so the backend can report syntax", () => {
    expect(isPromptTemplateText("画面 {{scene.name")).toBe(true);
    expect(extractPromptTemplateVariables("画面 {{scene.name")).toEqual([]);
  });
});
