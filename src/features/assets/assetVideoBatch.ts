import type { AssetView } from "../../types/asset";
import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { MINIMAX_H3_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { validateResolution } from "../runtime/resolution";

export const H3_PROMPT_KEY = "prompt" as const;
export const H3_REFERENCE_IMAGE_KEY = "reference_image" as const;
export const H3_DURATION_KEY = "duration_seconds" as const;
export const H3_SEED_KEY = "seed" as const;

type H3PromptField = Extract<RecipeField, { type: "textarea" }>;
type H3ReferenceField = Extract<RecipeField, { type: "image" | "images" }>;
type H3DurationField = Extract<RecipeField, { type: "integer" }>;
type H3ResolutionField = Extract<RecipeField, { type: "integer" }>;
type H3SeedField = Extract<RecipeField, { type: "seed" }>;

export interface H3RecipeContract {
  promptField: H3PromptField;
  referenceField: H3ReferenceField;
  widthField: H3ResolutionField;
  heightField: H3ResolutionField;
  durationField: H3DurationField;
  seedField: H3SeedField;
  durationOptions: number[];
}

export type H3RecipeContractResult =
  | { ok: true; contract: H3RecipeContract }
  | { ok: false; reason: string };

export function isImageAssetForVideo(asset: AssetView): boolean {
  return asset.assetType === "image";
}

export function splitPromptBlocks(input: string): string[] {
  return input
    .split(/\r?\n\s*\r?\n/)
    .map((prompt) => prompt.trim())
    .filter(Boolean);
}

export function buildH3BatchValues(
  recipe: RecipeViewModel,
  assetId: string,
  promptText: string,
  durationSeconds?: number,
  width?: number,
  height?: number,
): GenerationValues {
  const result = h3RecipeContract(recipe);
  if (!result.ok) throw new Error(result.reason);
  const defaultDuration = result.contract.durationField.default;
  if (defaultDuration === undefined) throw new Error("H3 Recipe 缺少 duration_seconds 默认值。");
  const duration = durationSeconds ?? defaultDuration;
  if (!result.contract.durationOptions.includes(duration)) {
    throw new Error(`H3 视频时长必须选择 ${result.contract.durationField.min}–${result.contract.durationField.max} 秒。`);
  }
  const selectedWidth = width ?? result.contract.widthField.default ?? result.contract.widthField.min;
  const selectedHeight = height ?? result.contract.heightField.default ?? result.contract.heightField.min;
  if (!validateResolution(recipe, selectedWidth, selectedHeight).ok) {
    throw new Error("H3 输出分辨率不符合当前 Recipe 约束。");
  }

  const values: GenerationValues = {};
  for (const field of recipe.fields) {
    const value = defaultValueForField(field);
    if (value) values[field.key] = value;
  }
  values[result.contract.promptField.key] = { type: "string", value: promptText.trim() };
  values[result.contract.widthField.key] = { type: "integer", value: selectedWidth! };
  values[result.contract.heightField.key] = { type: "integer", value: selectedHeight! };
  values[result.contract.durationField.key] = { type: "integer", value: duration };
  if (result.contract.referenceField) {
    values[result.contract.referenceField.key] = result.contract.referenceField.type === "images"
      ? { type: "image_assets", assetIds: [assetId] }
      : { type: "image_asset", assetId };
  }
  return values;
}

export function h3PromptField(recipe: RecipeViewModel): RecipeField | undefined {
  const result = h3RecipeContract(recipe);
  return result.ok ? result.contract.promptField : undefined;
}

export function h3ReferenceField(recipe: RecipeViewModel): RecipeField | undefined {
  const result = h3RecipeContract(recipe);
  return result.ok ? result.contract.referenceField : undefined;
}

export function h3RecipeContract(recipe: RecipeViewModel): H3RecipeContractResult {
  if (recipe.workflowId !== MINIMAX_H3_WORKFLOW_ID) {
    return { ok: false, reason: "运行目录中的 Recipe 不是 MiniMax H3。" };
  }
  if (!recipe.outputTypes?.includes("video")) {
    return { ok: false, reason: "H3 Recipe 未声明视频输出。" };
  }
  const promptField = exactField(recipe, H3_PROMPT_KEY, "textarea");
  if (!promptField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `prompt` 的 textarea 字段。" };
  }
  const referenceField = exactField(recipe, H3_REFERENCE_IMAGE_KEY, "image")
    ?? exactField(recipe, H3_REFERENCE_IMAGE_KEY, "images");
  if (!referenceField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `reference_image` 的 image/images 字段。" };
  }
  const durationField = exactField(recipe, H3_DURATION_KEY, "integer");
  if (!durationField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `duration_seconds` 的 integer 字段。" };
  }
  const widthField = exactField(recipe, "width", "integer");
  if (!widthField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `width` 的 integer 字段。" };
  }
  const heightField = exactField(recipe, "height", "integer");
  if (!heightField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `height` 的 integer 字段。" };
  }
  if (
    !widthField.required
    || !heightField.required
    || widthField.default === undefined
    || heightField.default === undefined
  ) {
    return { ok: false, reason: "H3 Recipe 的 width、height 必须是带默认值的必填字段。" };
  }
  if (
    durationField.min === undefined
    || durationField.max === undefined
    || durationField.default === undefined
    || !Number.isInteger(durationField.min)
    || !Number.isInteger(durationField.max)
    || !Number.isInteger(durationField.default)
    || durationField.step !== 1
    || durationField.min < 1
    || durationField.max > 15
    || durationField.min > durationField.max
    || durationField.default < durationField.min
    || durationField.default > durationField.max
  ) {
    return { ok: false, reason: "H3 Recipe 的 duration_seconds 必须是 1–15 秒、步长 1 且包含默认值。" };
  }
  const minDuration = durationField.min;
  const maxDuration = durationField.max;
  const seedField = exactField(recipe, H3_SEED_KEY, "seed");
  if (!seedField) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `seed` 的 seed 字段。" };
  }
  return {
    ok: true,
    contract: {
      promptField,
      referenceField,
      widthField,
      heightField,
      durationField,
      seedField,
      durationOptions: Array.from(
        { length: Math.floor((maxDuration - minDuration) / durationField.step!) + 1 },
        (_, index) => minDuration + index * durationField.step!,
      ),
    },
  };
}

function exactField<T extends RecipeField["type"]>(
  recipe: RecipeViewModel,
  key: string,
  type: T,
): Extract<RecipeField, { type: T }> | undefined {
  return recipe.fields.find((field) => field.key === key && field.type === type) as
    | Extract<RecipeField, { type: T }>
    | undefined;
}

function defaultValueForField(field: RecipeField): DraftValue | undefined {
  switch (field.type) {
    case "textarea":
      return { type: "string", value: field.default };
    case "integer":
      return field.default === undefined && field.min === undefined
        ? undefined
        : { type: "integer", value: field.default ?? field.min! };
    case "seed":
      return field.defaultMode === "fixed" && field.defaultValue
        ? { type: "seed_fixed", value: field.defaultValue }
        : { type: "seed_random" };
    case "images":
      return { type: "image_assets", assetIds: [] };
    case "image":
      return undefined;
    case "video":
    case "videos":
    case "audio":
    case "audios":
      return undefined;
  }
}
