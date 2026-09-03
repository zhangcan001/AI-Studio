import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowBindingView } from "../../types/projectWorkflow";
import type { ShotStage, ShotStageConfig } from "../../types/shot";
import { findRecipe, recipeRef } from "./workflowCapabilities";

export type ShotVideoInputMode = "TEXT_ONLY" | "SINGLE_IMAGE" | "REFERENCE_IMAGES" | "UNSUPPORTED";

export interface ShotStageRecipeCompatibility {
  compatible: boolean;
  stage: ShotStage;
  videoInputMode?: ShotVideoInputMode;
  reason?: string;
}

export type ShotStageRecipeSource = "stage_config" | "project_default" | "legacy";

export interface ShotStageRecipeResolution {
  recipe?: RecipeViewModel;
  source: ShotStageRecipeSource;
  blocked: boolean;
  reason?: string;
}

type ImageField = Extract<RecipeField, { type: "image" | "images" }>;
type UnsupportedMediaField = Extract<RecipeField, { type: "audio" | "audios" | "video" | "videos" }>;

function imageFields(recipe: RecipeViewModel): ImageField[] {
  return recipe.fields.filter((field): field is ImageField => field.type === "image" || field.type === "images");
}

function requiredUnsupportedMedia(recipe: RecipeViewModel): UnsupportedMediaField[] {
  return recipe.fields.filter((field): field is UnsupportedMediaField => (
    (field.type === "audio" || field.type === "audios" || field.type === "video" || field.type === "videos")
      && field.required
  ));
}

function unsupportedMediaReason(fields: UnsupportedMediaField[]): string {
  return `SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT: 当前 Shot 无法提供必填媒体输入：${fields.map((field) => field.key).join("、")}。`;
}

function multipleImageReason(stage: ShotStage): string {
  return `SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT: ${stage === "video" ? "视频" : "图片"}阶段只能表达一个 image 或 images 输入。`;
}

export function shotStageRecipeCompatibility(
  recipe: RecipeViewModel,
  stage: ShotStage,
): ShotStageRecipeCompatibility {
  const expectedOutput = stage === "image" ? "image" : "video";
  if (!recipe.outputTypes?.includes(expectedOutput)) {
    return {
      compatible: false,
      stage,
      ...(stage === "video" ? { videoInputMode: "UNSUPPORTED" as const } : {}),
      reason: `该 Recipe 没有 ${expectedOutput} 输出。`,
    };
  }

  const unsupported = requiredUnsupportedMedia(recipe);
  if (unsupported.length) {
    return {
      compatible: false,
      stage,
      ...(stage === "video" ? { videoInputMode: "UNSUPPORTED" as const } : {}),
      reason: unsupportedMediaReason(unsupported),
    };
  }

  const images = imageFields(recipe);
  if (images.length > 1) {
    return {
      compatible: false,
      stage,
      ...(stage === "video" ? { videoInputMode: "UNSUPPORTED" as const } : {}),
      reason: multipleImageReason(stage),
    };
  }

  if (stage === "image") {
    return { compatible: true, stage };
  }

  const videoInputMode: ShotVideoInputMode = images[0]?.type === "image"
    ? "SINGLE_IMAGE"
    : images[0]?.type === "images"
      ? "REFERENCE_IMAGES"
      : "TEXT_ONLY";
  return { compatible: true, stage, videoInputMode };
}

function bindingReason(source: Exclude<ShotStageRecipeSource, "legacy">, reason: string): string {
  return `${source === "project_default" ? "当前项目默认" : "当前 Shot 配置"}的工作流无法用于此阶段：${reason}`;
}

export function resolveShotStageRecipe(
  catalog: RecipeViewModel[],
  stage: ShotStage,
  stageConfig: Pick<ShotStageConfig, "workflowVersionId" | "recipeId"> | undefined,
  projectDefault: Pick<ProjectWorkflowBindingView, "workflowVersionId" | "recipeId" | "available"> | null | undefined,
  legacyFallback?: RecipeViewModel,
): ShotStageRecipeResolution {
  const source: ShotStageRecipeSource = stageConfig ? "stage_config" : projectDefault ? "project_default" : "legacy";
  const binding = stageConfig ?? projectDefault;

  if (binding) {
    const boundSource: Exclude<ShotStageRecipeSource, "legacy"> = stageConfig ? "stage_config" : "project_default";
    const recipe = findRecipe(catalog, recipeRef(binding));
    if (source === "project_default" && projectDefault?.available === false) {
      return {
        recipe,
        source,
        blocked: true,
        reason: bindingReason(boundSource, "项目绑定已标记为不可用，请重新选择项目默认工作流。"),
      };
    }
    if (!recipe) {
      return {
        source,
        blocked: true,
        reason: bindingReason(boundSource, "WorkflowVersion / Recipe 不在当前运行目录中，请重新选择工作流。"),
      };
    }
    const compatibility = shotStageRecipeCompatibility(recipe, stage);
    if (!compatibility.compatible) {
      return {
        recipe,
        source,
        blocked: true,
        reason: bindingReason(boundSource, compatibility.reason ?? "正式能力不兼容，请重新选择工作流。"),
      };
    }
    return { recipe, source, blocked: false };
  }

  if (!legacyFallback) {
    return {
      source,
      blocked: true,
      reason: `当前没有可用于${stage === "image" ? "图片" : "视频"}阶段的正式工作流。`,
    };
  }
  const compatibility = shotStageRecipeCompatibility(legacyFallback, stage);
  return compatibility.compatible
    ? { recipe: legacyFallback, source, blocked: false }
    : { recipe: legacyFallback, source, blocked: true, reason: compatibility.reason };
}

export function validateShotReferenceImages(
  field: Extract<RecipeField, { type: "images" }> | undefined,
  assetIds: string[],
): string | undefined {
  if (!field) return "SHOT_WORKFLOW_UNSUPPORTED_MEDIA_INPUT: Recipe 缺少 images 输入。";
  if (new Set(assetIds).size !== assetIds.length) return "参考图不能重复。";
  if (assetIds.length < field.minItems) return `参考图至少需要 ${field.minItems} 张。`;
  if (assetIds.length > field.maxItems) return `参考图最多允许 ${field.maxItems} 张。`;
  return undefined;
}
