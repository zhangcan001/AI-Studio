import { describe, expect, it } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import {
  applyPromptSnippetToStudio,
  applyPromptVersionToStudio,
  comparePromptVersions,
  createPromptVersion,
  normalizePromptText,
  promptVersionValues,
  selectPromptTargetField,
} from "./promptLibrary";

const recipe: RecipeViewModel = {
  workflowId: "wfl_prompt",
  workflowVersionId: "wfv_prompt",
  recipeId: "rcp_prompt",
  name: "Prompt test",
  category: "image",
  mode: "text_to_image",
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "steps", type: "integer", label: "步数", required: true, min: 1, max: 20, default: 8 },
  ],
};

describe("Pack 07 prompt library source contracts", () => {
  it("creates sequential normalized versions and compares changed lines", () => {
    const first = createPromptVersion("prompt-1", [], "人物\r\n柔光", "2026-08-10T00:00:00Z", "pv-1");
    const second = createPromptVersion("prompt-1", [first], "人物\n硬光", "2026-08-10T00:01:00Z", "pv-2");
    expect(first.version).toBe(1);
    expect(second.version).toBe(2);
    expect(normalizePromptText(first.text)).toBe("人物\n柔光");
    expect(comparePromptVersions(first, second)).toMatchObject({
      fromVersion: 1,
      toVersion: 2,
      removedLines: ["柔光"],
      addedLines: ["硬光"],
      changed: true,
    });
  });

  it("applies a prompt version only to an exact textarea field", () => {
    const version = createPromptVersion("prompt-1", [], "新的提示词", "2026-08-10T00:00:00Z", "pv-1");
    const values: GenerationValues = { steps: { type: "integer", value: 8 } };
    const applied = applyPromptVersionToStudio(recipe, values, "prompt", version);
    expect(applied.values?.prompt).toEqual({ type: "string", value: "新的提示词" });
    expect(applyPromptVersionToStudio(recipe, values, "steps", version).issue).toBeTruthy();
    expect(promptVersionValues([version])).toEqual([{ type: "string", value: "新的提示词" }]);
  });

  it("applies snippets with explicit prepend, append, and replace semantics", () => {
    const values: GenerationValues = {
      prompt: { type: "string", value: "base" },
    };
    expect(applyPromptSnippetToStudio(recipe, values, "prompt", "head", "prepend").values?.prompt)
      .toEqual({ type: "string", value: "head\nbase" });
    expect(applyPromptSnippetToStudio(recipe, values, "prompt", "tail", "append").values?.prompt)
      .toEqual({ type: "string", value: "base\ntail" });
    expect(applyPromptSnippetToStudio(recipe, values, "prompt", "new", "replace").values?.prompt)
      .toEqual({ type: "string", value: "new" });
  });

  it("requires an explicit target when a Recipe has multiple textareas", () => {
    const multiTextareaRecipe = {
      ...recipe,
      fields: [
        ...recipe.fields,
        { key: "negative", type: "textarea" as const, label: "反向提示词", required: false, default: "" },
      ],
    };
    expect(selectPromptTargetField(multiTextareaRecipe).issue).toContain("多个文字输入字段");
    expect(selectPromptTargetField(multiTextareaRecipe, "missing").issue).toContain("不属于当前 Recipe");
    expect(selectPromptTargetField(multiTextareaRecipe, "negative").fieldKey).toBe("negative");
  });
});
