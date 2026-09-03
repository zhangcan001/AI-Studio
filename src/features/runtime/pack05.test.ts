import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import {
  applyRuntimeParameterProfile,
  evaluateRuntimeGate,
  filterRuntimeCatalog,
  inspectWorkflowImport,
  migrateLegacyRuntimeProfile,
  runtimeKindFor,
  sanitizeRuntimeParameterValues,
  validateMultiRuntimeQueue,
  type RuntimeParameterProfile,
} from "./pack05";

const imageRecipe: RecipeViewModel = {
  workflowId: "image-runtime",
  workflowVersionId: "image-v1",
  recipeId: "image-recipe",
  name: "通用图片工作流",
  category: "image",
  mode: "image_to_image",
  fields: [
    { key: "prompt", type: "textarea", label: "提示词", required: false, default: "" },
    { key: "steps", type: "integer", label: "Steps", required: true, min: 1, max: 30, default: 4 },
    { key: "width", type: "integer", label: "Width", required: true, min: 256, max: 2048, default: 1024 },
    { key: "height", type: "integer", label: "Height", required: true, min: 256, max: 2048, default: 1024 },
    { key: "reference", type: "image", label: "参考图", required: true },
  ],
};

describe("M3 Pack 05 runtime contracts", () => {
  it("classifies and filters runtimes from category and mode metadata", () => {
    expect(runtimeKindFor(imageRecipe)).toBe("image");
    expect(runtimeKindFor({ category: "video", mode: "reference_to_video" })).toBe("video");
    expect(filterRuntimeCatalog([imageRecipe, { ...imageRecipe, workflowVersionId: "video-v1", category: "video", mode: "text_to_video" }], "video")).toHaveLength(1);
  });

  it("keeps only safe integer values without model-specific bounds", () => {
    expect(sanitizeRuntimeParameterValues({ steps: 999, width: 1, durationSeconds: 0, tokenizer: 4.5 })).toEqual({ steps: 999, width: 1, durationSeconds: 0 });
  });

  it("applies only parameters that have matching integer recipe fields", () => {
    const profile: RuntimeParameterProfile = {
      id: "profile-1",
      workflowVersionId: imageRecipe.workflowVersionId,
      recipeId: imageRecipe.recipeId,
      name: "预览",
      values: { steps: 50, width: 4096, height: 768 },
      updatedAt: "2026-08-10T00:00:00.000Z",
    };
    const result = applyRuntimeParameterProfile(imageRecipe, {}, profile);
    expect(result.values.steps).toEqual({ type: "integer", value: 30 });
    expect(result.values.width).toEqual({ type: "integer", value: 2048 });
    expect(result.ignoredParameters).toEqual([]);
  });

  it("binds profiles by exact integer field key and ignores unknown keys", () => {
    const result = applyRuntimeParameterProfile(imageRecipe, {}, {
      id: "profile-2",
      workflowVersionId: imageRecipe.workflowVersionId,
      recipeId: imageRecipe.recipeId,
      name: "直接绑定",
      values: { steps: 12, tokenizer: 4 },
      updatedAt: "2026-08-10T00:00:00.000Z",
    });
    expect(result.values.steps).toEqual({ type: "integer", value: 12 });
    expect(result.ignoredParameters).toEqual(["tokenizer"]);
  });

  it("migrates legacy semantic keys with explicit aliases and ignores concurrency", () => {
    const result = migrateLegacyRuntimeProfile(imageRecipe, {
      id: "legacy-1",
      workflowVersionId: imageRecipe.workflowVersionId,
      recipeId: imageRecipe.recipeId,
      name: "旧档案",
      values: { steps: 12, durationSeconds: 5, concurrency: 4 },
      updatedAt: "2026-08-10T00:00:00.000Z",
    });
    expect(result.unresolvedKeys).toEqual(["durationSeconds"]);
    expect(result.profile).toBeUndefined();
  });

  it("rejects UI-format workflow files while accepting API-format graphs", () => {
    const ui = inspectWorkflowImport(JSON.stringify({ nodes: [{ id: 1, type: "SaveImage" }], links: [] }));
    expect(ui.accepted).toBe(false);
    expect(ui.format).toBe("ui");
    const api = inspectWorkflowImport(JSON.stringify({ "1": { class_type: "SaveImage", inputs: {} } }));
    expect(api.accepted).toBe(true);
    expect(api.nodeCount).toBe(1);
  });

  it("flags credential-shaped workflow fields before import", () => {
    const report = inspectWorkflowImport(JSON.stringify({ "1": { class_type: "CustomNode", inputs: { api_key: "secret" } } }));
    expect(report.accepted).toBe(false);
    expect(report.errors).toContain("工作流包含疑似凭据字段，请移除凭据后再导入。");
  });

  it("does not flag tokenizer or token_count as credential keys", () => {
    const report = inspectWorkflowImport(JSON.stringify({
      "1": { class_type: "CustomNode", inputs: { tokenizer: "clip", token_count: 12 } },
    }));
    expect(report.accepted).toBe(true);
  });

  it("validates a queue across multiple ready runtimes in order", () => {
    const valid = validateMultiRuntimeQueue(
      [{ id: "one", runtimeId: "a" }, { id: "two", runtimeId: "b" }],
      [{ id: "a", enabled: true, readiness: "READY", capability: "READY" }, { id: "b", enabled: true, readiness: "READY", capability: "READY" }],
      { requireMultipleRuntimes: true },
    );
    expect(valid.valid).toBe(true);
    const invalid = validateMultiRuntimeQueue([{ id: "one", runtimeId: "a" }], [{ id: "a", enabled: true, readiness: "BLOCKED" }], { requireMultipleRuntimes: true });
    expect(invalid.valid).toBe(false);
    expect(invalid.issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["QUEUE_NOT_MULTI_RUNTIME", "QUEUE_RUNTIME_NOT_READY"]));
  });

  it("separates missing local runtime input from code blockers at the release gate", () => {
    expect(evaluateRuntimeGate({ packageImported: false, capabilityReady: false, quickTestPassed: false, outputVerified: false, historyAvailable: false, environmentInputAvailable: false, environmentBlockCode: "THIRD_RUNTIME_INPUT_REQUIRED" })).toEqual({ status: "ENVIRONMENT_BLOCKED", issues: ["THIRD_RUNTIME_INPUT_REQUIRED"] });
    expect(evaluateRuntimeGate({ packageImported: true, capabilityReady: true, quickTestPassed: false, outputVerified: true, historyAvailable: true })).toEqual({ status: "BLOCKED", issues: ["RUNTIME_QUICK_TEST_FAILED"] });
    expect(evaluateRuntimeGate({ packageImported: true, capabilityReady: true, quickTestPassed: true, outputVerified: true, historyAvailable: true })).toEqual({ status: "PASS", issues: [] });
  });
});
