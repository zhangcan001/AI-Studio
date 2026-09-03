import { describe, expect, it } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type {
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
  ProjectWorkflowMode,
} from "../../types/projectWorkflow";
import {
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
  KERA2_WORKFLOW_ID,
} from "./productRuntimeScope";
import {
  preflightProjectWorkflow,
  PROJECT_WORKFLOW_VIDEO_MODES,
} from "./projectWorkflowPreflight";

function promptField(): RecipeField {
  return { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };
}

function mediaField(key: string, type: "image" | "video" | "audio"): RecipeField {
  return { key, type, label: key, required: false };
}

function videoRecipe(
  id: string,
  mediaKeys: string[] = [],
  workflowId = `workflow-${id}`,
): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: `Video ${id}`,
    category: "video",
    mode: "video",
    fields: [
      promptField(),
      ...mediaKeys.map((key) => mediaField(key, key.includes("audio") ? "audio" : key.includes("video") ? "video" : "image")),
    ],
    outputTypes: ["video"],
  };
}

function imageRecipe(id = "image"): RecipeViewModel {
  return {
    workflowId: KERA2_WORKFLOW_ID,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: "Krea Image",
    category: "image",
    mode: "text_to_image",
    fields: [
      promptField(),
      { key: "width", type: "integer", label: "Width", required: true, default: 1024 },
      { key: "height", type: "integer", label: "Height", required: true, default: 1024 },
      { key: "seed", type: "seed", label: "Seed", defaultMode: "random" },
    ],
    outputTypes: ["image"],
  };
}

function binding(
  stage: "IMAGE" | "VIDEO",
  mode: ProjectWorkflowMode,
  id: string,
  available = true,
): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    available,
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: "project-1", videoModeOverrides: [], ...patch };
}

function item(report: ReturnType<typeof preflightProjectWorkflow>, path: typeof PROJECT_WORKFLOW_VIDEO_MODES[number] | "IMAGE") {
  return report.items.find((candidate) => candidate.path === path)!;
}

describe("project workflow preflight", () => {
  it("reports all eight paths and honors mode override before video default", () => {
    const image = imageRecipe();
    const videoDefault = videoRecipe("default", ["first_frame", "last_frame", "reference_image", "reference_audio", "reference_video"]);
    const modeOverride = videoRecipe("override", ["first_frame"]);
    const report = preflightProjectWorkflow(config({
      imageDefault: binding("IMAGE", "DEFAULT", "image"),
      videoDefault: binding("VIDEO", "DEFAULT", "default"),
      videoModeOverrides: [binding("VIDEO", "FL2VA_IMAGE_TO_VIDEO", "override")],
    }), [image, videoDefault, modeOverride]);

    expect(report.items).toHaveLength(8);
    expect(report.overallStatus).toBe("READY");
    expect(report.readyCount).toBe(8);
    expect(item(report, "IMAGE")).toMatchObject({ status: "READY", source: "project_default", usingFallback: false });
    expect(item(report, "FL2VA_TEXT_TO_VIDEO")).toMatchObject({ recipe: videoDefault, source: "project_default" });
    expect(item(report, "FL2VA_IMAGE_TO_VIDEO")).toMatchObject({ recipe: modeOverride, source: "project_mode", usingFallback: false });
  });

  it("uses the H3 recommendation before the first compatible recipe", () => {
    const recommended = videoRecipe(
      "recommended",
      ["reference_image", "reference_audio", "reference_video"],
      MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
    );
    const compatible = videoRecipe("compatible", ["first_frame"]);
    const report = preflightProjectWorkflow(config(), [imageRecipe(), recommended, compatible]);

    expect(item(report, "REF2VA_IMAGE")).toMatchObject({ recipe: recommended, source: "recommended" });
    expect(item(report, "FL2VA_IMAGE_TO_VIDEO")).toMatchObject({ recipe: recommended, source: "compatible" });
  });

  it("warns on a stale mode override while keeping the project default", () => {
    const videoDefault = videoRecipe("default", ["first_frame", "last_frame", "reference_image", "reference_audio", "reference_video"]);
    const stale = binding("VIDEO", "FL2VA_IMAGE_TO_VIDEO", "stale", false);
    const report = preflightProjectWorkflow(config({
      videoDefault: binding("VIDEO", "DEFAULT", "default"),
      videoModeOverrides: [stale],
    }), [imageRecipe(), videoDefault]);
    const result = item(report, "FL2VA_IMAGE_TO_VIDEO");

    expect(result).toMatchObject({
      status: "WARNING",
      recipe: videoDefault,
      source: "project_default",
      configuredRef: { workflowVersionId: "version-stale", recipeId: "recipe-stale" },
      staleConfiguredBinding: true,
      usingFallback: true,
    });
    expect(report.overallStatus).toBe("READY");
    expect(report.warningCount).toBe(1);
  });

  it("does not call an incompatible but available video default stale for another mode", () => {
    const textToVideo = videoRecipe("t2v");
    const imageToVideo = videoRecipe("i2v", ["first_frame"]);
    const report = preflightProjectWorkflow(config({
      videoDefault: binding("VIDEO", "DEFAULT", "t2v"),
    }), [imageRecipe(), textToVideo, imageToVideo]);
    const result = item(report, "FL2VA_IMAGE_TO_VIDEO");

    expect(result).toMatchObject({
      status: "READY",
      recipe: imageToVideo,
      source: "compatible",
      staleConfiguredBinding: false,
      usingFallback: true,
    });
  });

  it("warns on a stale video default and uses the mode recommendation", () => {
    const recommended = videoRecipe(
      "recommended",
      ["reference_image", "reference_audio"],
      MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
    );
    const report = preflightProjectWorkflow(config({
      videoDefault: binding("VIDEO", "DEFAULT", "missing", false),
    }), [imageRecipe(), recommended]);
    const result = item(report, "REF2VA_IMAGE");

    expect(result).toMatchObject({
      status: "WARNING",
      recipe: recommended,
      source: "recommended",
      staleConfiguredBinding: true,
      usingFallback: true,
    });
  });

  it("reports PARTIAL when only some paths have compatible recipes", () => {
    const report = preflightProjectWorkflow(config(), [imageRecipe(), videoRecipe("t2v")]);

    expect(report.overallStatus).toBe("PARTIAL");
    expect(report.readyCount).toBe(2);
    expect(report.blockedCount).toBe(6);
    expect(item(report, "REF2VA_AUDIO").status).toBe("BLOCKED");
    expect(item(report, "REF2VA_AUDIO").recipe).toBeUndefined();
  });

  it("reports BLOCKED only when every production path lacks a recipe", () => {
    const report = preflightProjectWorkflow(config(), []);

    expect(report.overallStatus).toBe("BLOCKED");
    expect(report.readyCount).toBe(0);
    expect(report.blockedCount).toBe(8);
    expect(report.items.every((candidate) => candidate.status === "BLOCKED")).toBe(true);
  });

  it("marks a stale image default as WARNING when the image recommendation remains available", () => {
    const report = preflightProjectWorkflow(config({
      imageDefault: binding("IMAGE", "DEFAULT", "missing", false),
    }), [imageRecipe()]);
    const result = item(report, "IMAGE");

    expect(result).toMatchObject({
      status: "WARNING",
      source: "recommended",
      staleConfiguredBinding: true,
      usingFallback: true,
    });
  });
});
