import type { AssetView } from "../../types/asset";
import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { h3FamilyForWorkflowId } from "../runtime/productRuntimeScope";
import { validateResolution } from "../runtime/resolution";
import { isMinimaxH3OutputResolution } from "../runtime/resolutionPresets";
import type { BatchDraftItem } from "../studio/batchDraft";

export const H3_PROMPT_KEY = "prompt" as const;
export const H3_REFERENCE_IMAGE_KEY = "reference_image" as const;
export const H3_REFERENCE_IMAGES_KEY = "reference_images" as const;
export const H3_REFERENCE_VIDEOS_KEY = "reference_videos" as const;
export const H3_REFERENCE_AUDIOS_KEY = "reference_audios" as const;
export const H3_FIRST_FRAME_KEY = "first_frame" as const;
export const H3_LAST_FRAME_KEY = "last_frame" as const;
export const H3_DURATION_KEY = "duration_seconds" as const;
export const H3_SEED_KEY = "seed" as const;

type H3PromptField = Extract<RecipeField, { type: "textarea" }>;
type H3ReferenceField = Extract<RecipeField, { type: "image" | "images" }>;
type H3ImageField = Extract<RecipeField, { type: "image" }>;
type H3ImagesField = Extract<RecipeField, { type: "images" }>;
type H3VideosField = Extract<RecipeField, { type: "videos" }>;
type H3AudiosField = Extract<RecipeField, { type: "audios" }>;
type H3DurationField = Extract<RecipeField, { type: "integer" }>;
type H3ResolutionField = Extract<RecipeField, { type: "integer" }>;
type H3SeedField = Extract<RecipeField, { type: "seed" }>;

export interface H3RecipeContract {
  promptField: H3PromptField;
  referenceField?: H3ReferenceField;
  firstFrameField?: H3ImageField;
  lastFrameField?: H3ImageField;
  referenceImagesField?: H3ImagesField;
  referenceVideosField?: H3VideosField;
  referenceAudiosField?: H3AudiosField;
  widthField: H3ResolutionField;
  heightField: H3ResolutionField;
  durationField: H3DurationField;
  seedField: H3SeedField;
  durationOptions: number[];
  family: "FL2VA" | "REF2VA";
}

export type H3GenerationMode =
  | "FL2VA_TEXT_TO_VIDEO"
  | "FL2VA_IMAGE_TO_VIDEO"
  | "FL2VA_FIRST_LAST"
  | "REF2VA_IMAGE"
  | "REF2VA_AUDIO"
  | "REF2VA_IMAGE_AUDIO"
  | "REF2VA_VIDEO_IMAGE";

export interface H3ModeAssets {
  firstFrameAssetId?: string;
  lastFrameAssetId?: string;
  imageAssetIds?: string[];
  videoAssetIds?: string[];
  audioAssetIds?: string[];
}

export interface H3ModeOption {
  id: H3GenerationMode;
  label: string;
  description: string;
  family: "FL2VA" | "REF2VA";
}

export const H3_MODE_OPTIONS: H3ModeOption[] = [
  { id: "FL2VA_TEXT_TO_VIDEO", label: "文生视频", description: "只用 Prompt 生成视频", family: "FL2VA" },
  { id: "FL2VA_IMAGE_TO_VIDEO", label: "一张图生视频", description: "从首帧图片开始生成", family: "FL2VA" },
  { id: "FL2VA_FIRST_LAST", label: "首尾帧视频", description: "锁定首帧与末帧", family: "FL2VA" },
  { id: "REF2VA_IMAGE", label: "仅图片", description: "使用有序图片参考", family: "REF2VA" },
  { id: "REF2VA_AUDIO", label: "仅音频", description: "使用有序音频参考", family: "REF2VA" },
  { id: "REF2VA_IMAGE_AUDIO", label: "图片 + 音频", description: "同时使用图片和音频", family: "REF2VA" },
  { id: "REF2VA_VIDEO_IMAGE", label: "视频 + 图片", description: "使用视频帧、原生音频和图片", family: "REF2VA" },
];

export interface H3AssetQualificationInput {
  isImage: boolean;
  promptReady: boolean;
  promptTooLong: boolean;
  h3RuntimeReady: boolean;
  comfyConnected: boolean;
  taskEventsReady: boolean;
  durationReady: boolean;
  resolutionReady: boolean;
  resolutionError?: string;
}

export interface H3BatchCreationEligibilityInput {
  runtimeReady: boolean;
  admissionBusy: boolean;
  imageCount: number;
  missingPromptCount: number;
  oversizedPromptCount: number;
}

export type H3RecipeContractResult =
  | { ok: true; contract: H3RecipeContract }
  | { ok: false; reason: string };

export interface H3BatchDraftInput {
  recipe?: RecipeViewModel;
  contract: H3RecipeContractResult;
  mode: H3GenerationMode;
  prompt: string;
  promptTooLong: boolean;
  durationSeconds?: number;
  width?: number;
  height?: number;
  durationReady: boolean;
  resolutionReady: boolean;
  modeSupported: boolean;
  modeAssetReady: boolean;
  firstFrameAssetId?: string;
  lastFrameAssetId?: string;
  imageAssetIds?: string[];
  videoAssetIds?: string[];
  audioAssetIds?: string[];
}

export interface H3BatchDraftResult {
  items: BatchDraftItem[];
  error?: string;
}

export function isImageAssetForVideo(asset: AssetView): boolean {
  return asset.assetType === "image";
}

export function isVideoAssetForH3(asset: AssetView): boolean {
  return asset.assetType === "video";
}

export function isAudioAssetForH3(asset: AssetView): boolean {
  return asset.assetType === "audio";
}

export function h3AssetQualification(input: H3AssetQualificationInput): string {
  if (!input.isImage) return "不是图片素材";
  if (!input.promptReady) return "未填写视频提示词";
  if (input.promptTooLong) return "视频提示词超过 64 KiB";
  if (!input.h3RuntimeReady) return "H3 runtime unavailable";
  if (!input.comfyConnected) return "ComfyUI 未连接";
  if (!input.taskEventsReady) return "任务事件通道未就绪";
  if (!input.durationReady) return "请选择有效时长";
  if (!input.resolutionReady) return input.resolutionError ?? "请选择有效分辨率";
  return "符合条件";
}

export function canCreateH3Batch(input: H3BatchCreationEligibilityInput): boolean {
  return input.runtimeReady
    && !input.admissionBusy
    && input.imageCount > 0
    && input.missingPromptCount === 0
    && input.oversizedPromptCount === 0
    && input.imageCount <= 100;
}

/**
 * Builds the preview queue only after all user-facing inputs are ready.
 * The strict builder below intentionally keeps throwing for invalid Task Truth;
 * this boundary prevents normal incomplete form state from throwing during render.
 */
export function buildH3BatchDraft(input: H3BatchDraftInput): H3BatchDraftResult {
  if (
    !input.recipe
    || !input.contract.ok
    || !input.modeSupported
    || !input.modeAssetReady
    || input.durationSeconds === undefined
    || input.width === undefined
    || input.height === undefined
    || !input.durationReady
    || !input.resolutionReady
    || !input.prompt.trim()
    || input.promptTooLong
  ) {
    return { items: [] };
  }

  const { recipe, mode, prompt, durationSeconds, width, height } = input;
  const build = (id: string, assets: H3ModeAssets): BatchDraftItem => ({
    id,
    workflowName: recipe.name,
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    values: buildH3ModeBatchValues(recipe, mode, prompt, assets, durationSeconds, width, height),
  });

  try {
    switch (mode) {
      case "FL2VA_IMAGE_TO_VIDEO":
        return { items: input.firstFrameAssetId ? [build(input.firstFrameAssetId, { firstFrameAssetId: input.firstFrameAssetId })] : [] };
      case "FL2VA_FIRST_LAST":
        return {
          items: input.firstFrameAssetId && input.lastFrameAssetId
            ? [build(`${input.firstFrameAssetId}:${input.lastFrameAssetId}`, {
              firstFrameAssetId: input.firstFrameAssetId,
              lastFrameAssetId: input.lastFrameAssetId,
            })]
            : [],
        };
      case "REF2VA_IMAGE":
        return { items: [build("reference-images", { imageAssetIds: input.imageAssetIds ?? [] })] };
      case "REF2VA_AUDIO":
        return { items: [build("reference-audios", { audioAssetIds: input.audioAssetIds ?? [] })] };
      case "REF2VA_IMAGE_AUDIO":
        return {
          items: [build("reference-images-audios", {
            imageAssetIds: input.imageAssetIds ?? [],
            audioAssetIds: input.audioAssetIds ?? [],
          })],
        };
      case "REF2VA_VIDEO_IMAGE":
        return {
          items: [build("reference-videos-images", {
            imageAssetIds: input.imageAssetIds ?? [],
            videoAssetIds: input.videoAssetIds ?? [],
          })],
        };
      case "FL2VA_TEXT_TO_VIDEO":
        return { items: [build("text-to-video", {})] };
    }
  } catch (error: unknown) {
    return { items: [], error: error instanceof Error ? error.message : String(error) };
  }
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
  if (!isMinimaxH3OutputResolution(selectedWidth!, selectedHeight!) || !validateResolution(recipe, selectedWidth, selectedHeight).ok) {
    throw new Error("H3 输出分辨率必须选择图片规格中的 14 档 16:9 分辨率。");
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
  } else if (result.contract.referenceImagesField) {
    values[result.contract.referenceImagesField.key] = { type: "image_assets", assetIds: [assetId] };
  } else if (result.contract.firstFrameField) {
    values[result.contract.firstFrameField.key] = { type: "image_asset", assetId };
  }
  return values;
}

export function buildH3ModeBatchValues(
  recipe: RecipeViewModel,
  mode: H3GenerationMode,
  promptText: string,
  assets: H3ModeAssets = {},
  durationSeconds?: number,
  width?: number,
  height?: number,
): GenerationValues {
  const result = h3RecipeContract(recipe);
  if (!result.ok) throw new Error(result.reason);
  const { contract } = result;
  const defaultDuration = contract.durationField.default;
  if (defaultDuration === undefined) throw new Error("H3 Recipe 缺少 duration_seconds 默认值。");
  const duration = durationSeconds ?? defaultDuration;
  if (!contract.durationOptions.includes(duration)) {
    throw new Error(`H3 视频时长必须选择 ${contract.durationField.min}–${contract.durationField.max} 秒。`);
  }
  const selectedWidth = width ?? contract.widthField.default ?? contract.widthField.min;
  const selectedHeight = height ?? contract.heightField.default ?? contract.heightField.min;
  if (!isMinimaxH3OutputResolution(selectedWidth!, selectedHeight!) || !validateResolution(recipe, selectedWidth, selectedHeight).ok) {
    throw new Error("H3 输出分辨率必须选择图片规格中的 14 档 16:9 分辨率。");
  }
  if (!h3ModeSupported(contract, mode)) {
    throw new Error(`当前 H3 Recipe 不支持模式 ${mode}。`);
  }

  const values: GenerationValues = {};
  for (const field of recipe.fields) {
    const value = defaultValueForField(field);
    if (value && field.type !== "image" && field.type !== "images" && field.type !== "video" && field.type !== "videos" && field.type !== "audio" && field.type !== "audios") {
      values[field.key] = value;
    }
  }
  values[contract.promptField.key] = { type: "string", value: promptText.trim() };
  values[contract.widthField.key] = { type: "integer", value: selectedWidth! };
  values[contract.heightField.key] = { type: "integer", value: selectedHeight! };
  values[contract.durationField.key] = { type: "integer", value: duration };

  switch (mode) {
    case "FL2VA_TEXT_TO_VIDEO":
      break;
    case "FL2VA_IMAGE_TO_VIDEO":
      setSingleImageValue(values, contract.firstFrameField, assets.firstFrameAssetId, "first_frame");
      break;
    case "FL2VA_FIRST_LAST":
      setSingleImageValue(values, contract.firstFrameField, assets.firstFrameAssetId, "first_frame");
      setSingleImageValue(values, contract.lastFrameField, assets.lastFrameAssetId, "last_frame");
      break;
    case "REF2VA_IMAGE":
      setReferenceImagesValue(values, contract, assets.imageAssetIds ?? []);
      break;
    case "REF2VA_AUDIO":
      setPluralValue(values, contract.referenceAudiosField, assets.audioAssetIds ?? [], "reference_audios");
      break;
    case "REF2VA_IMAGE_AUDIO":
      setReferenceImagesValue(values, contract, assets.imageAssetIds ?? []);
      setPluralValue(values, contract.referenceAudiosField, assets.audioAssetIds ?? [], "reference_audios");
      break;
    case "REF2VA_VIDEO_IMAGE":
      setPluralValue(values, contract.referenceVideosField, assets.videoAssetIds ?? [], "reference_videos");
      setReferenceImagesValue(values, contract, assets.imageAssetIds ?? []);
      break;
  }
  return values;
}

export function h3ModeSupported(contract: H3RecipeContract, mode: H3GenerationMode): boolean {
  switch (mode) {
    case "FL2VA_TEXT_TO_VIDEO":
      return contract.family === "FL2VA";
    case "FL2VA_IMAGE_TO_VIDEO":
      return contract.family === "FL2VA" && Boolean(contract.firstFrameField);
    case "FL2VA_FIRST_LAST":
      return contract.family === "FL2VA" && Boolean(contract.firstFrameField && contract.lastFrameField);
    case "REF2VA_IMAGE":
      return contract.family === "REF2VA" && Boolean(contract.referenceImagesField || contract.referenceField);
    case "REF2VA_AUDIO":
      return contract.family === "REF2VA" && Boolean(contract.referenceAudiosField);
    case "REF2VA_IMAGE_AUDIO":
      return contract.family === "REF2VA" && Boolean((contract.referenceImagesField || contract.referenceField) && contract.referenceAudiosField);
    case "REF2VA_VIDEO_IMAGE":
      return contract.family === "REF2VA" && Boolean(contract.referenceVideosField && (contract.referenceImagesField || contract.referenceField));
  }
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
  const family = h3FamilyForWorkflowId(recipe.workflowId);
  if (!family) {
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
  const firstFrameField = exactField(recipe, H3_FIRST_FRAME_KEY, "image");
  const lastFrameField = exactField(recipe, H3_LAST_FRAME_KEY, "image");
  const referenceImagesField = exactField(recipe, H3_REFERENCE_IMAGES_KEY, "images");
  const referenceVideosField = exactField(recipe, H3_REFERENCE_VIDEOS_KEY, "videos");
  const referenceAudiosField = exactField(recipe, H3_REFERENCE_AUDIOS_KEY, "audios");
  if (
    family === "REF2VA"
    && !referenceField
    && !referenceImagesField
    && !referenceVideosField
    && !referenceAudiosField
  ) {
    return { ok: false, reason: "H3 Recipe 缺少 key 为 `reference_image` 或 Omni Reference media 字段。" };
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
      firstFrameField,
      lastFrameField,
      referenceImagesField,
      referenceVideosField,
      referenceAudiosField,
      widthField,
      heightField,
      durationField,
      seedField,
      durationOptions: Array.from(
        { length: Math.floor((maxDuration - minDuration) / durationField.step!) + 1 },
        (_, index) => minDuration + index * durationField.step!,
      ),
      family,
    },
  };
}

function setSingleImageValue(
  values: GenerationValues,
  field: H3ImageField | undefined,
  assetId: string | undefined,
  label: string,
) {
  if (!field || !assetId) throw new Error(`${label} 模式需要图片素材。`);
  values[field.key] = { type: "image_asset", assetId };
}

function setPluralValue(
  values: GenerationValues,
  field: H3ImagesField | H3VideosField | H3AudiosField | undefined,
  assetIds: string[],
  label: string,
) {
  if (!field || !assetIds.length) throw new Error(`${label} 模式至少需要一个对应素材。`);
  const type = field.type === "images" ? "image_assets" : field.type === "videos" ? "video_assets" : "audio_assets";
  values[field.key] = { type, assetIds } as DraftValue;
}

function setReferenceImagesValue(
  values: GenerationValues,
  contract: H3RecipeContract,
  assetIds: string[],
) {
  if (contract.referenceImagesField) {
    setPluralValue(values, contract.referenceImagesField, assetIds, H3_REFERENCE_IMAGES_KEY);
    return;
  }
  if (!contract.referenceField || !assetIds.length) {
    throw new Error("REF2VA 图片模式至少需要一个图片素材。");
  }
  values[contract.referenceField.key] = contract.referenceField.type === "images"
    ? { type: "image_assets", assetIds }
    : { type: "image_asset", assetId: assetIds[0] };
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
