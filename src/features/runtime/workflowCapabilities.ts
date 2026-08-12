import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { h3ModeSupported, h3RecipeContract } from "../assets/assetVideoBatch";

export type H3CompatibleMode =
  | "FL2VA_TEXT_TO_VIDEO"
  | "FL2VA_IMAGE_TO_VIDEO"
  | "FL2VA_FIRST_LAST"
  | "REF2VA_IMAGE"
  | "REF2VA_AUDIO"
  | "REF2VA_IMAGE_AUDIO"
  | "REF2VA_VIDEO_IMAGE";

export type VideoCompatibleMode = H3CompatibleMode | "CUSTOM_VIDEO";

export interface SelectedRecipeRef {
  workflowVersionId: string;
  recipeId: string;
}

export interface ImageRecipeCapability {
  outputImage: boolean;
  promptField?: Extract<RecipeField, { type: "textarea" }>;
  batchPromptCompatible: boolean;
  genericGenerationCompatible: boolean;
  reason?: string;
}

export interface VideoRecipeCapability {
  outputVideo: boolean;
  promptField?: Extract<RecipeField, { type: "textarea" }>;
  supportedModes: VideoCompatibleMode[];
  projectFolderModes: H3CompatibleMode[];
  genericGenerationCompatible: boolean;
  reason?: string;
}

export type WorkflowSelectionSource = "manual" | "recommended" | "compatible";

export interface ResolvedRecipe<T extends RecipeViewModel = RecipeViewModel> {
  recipe?: T;
  source?: WorkflowSelectionSource;
  staleManualSelection: boolean;
}

const promptKeys = new Set(["prompt", "text_prompt", "positive_prompt"]);
const firstFrameKeys = new Set(["first_frame", "start_frame", "image", "input_image"]);
const lastFrameKeys = new Set(["last_frame", "end_frame"]);
const referenceImageKeys = new Set(["reference_image", "reference_images", "images", "image_references"]);
const referenceAudioKeys = new Set(["reference_audio", "reference_audios", "audios", "audio_references"]);
const referenceVideoKeys = new Set(["reference_video", "reference_videos", "videos", "video_references"]);

export function recipeRef(recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">): SelectedRecipeRef {
  return { workflowVersionId: recipe.workflowVersionId, recipeId: recipe.recipeId };
}

export function sameRecipeRef(
  left: Pick<SelectedRecipeRef, "workflowVersionId" | "recipeId"> | undefined,
  right: Pick<SelectedRecipeRef, "workflowVersionId" | "recipeId"> | undefined,
): boolean {
  return Boolean(
    left
      && right
      && left.workflowVersionId === right.workflowVersionId
      && left.recipeId === right.recipeId,
  );
}

export function imageRecipeCapability(recipe: RecipeViewModel): ImageRecipeCapability {
  const outputImage = recipe.outputTypes?.includes("image") ?? false;
  const promptField = findTextarea(recipe, promptKeys);
  const batchPromptCompatible = outputImage && Boolean(promptField);
  const genericGenerationCompatible = outputImage;

  if (!outputImage) {
    return {
      outputImage,
      promptField,
      batchPromptCompatible: false,
      genericGenerationCompatible: false,
      reason: "该 Recipe 没有图片输出。",
    };
  }
  if (!promptField) {
    return {
      outputImage,
      promptField,
      batchPromptCompatible,
      genericGenerationCompatible,
      reason: "该工作流没有可识别的标准 Prompt 输入，可使用通用参数模式生成。",
    };
  }
  return { outputImage, promptField, batchPromptCompatible, genericGenerationCompatible };
}

export function filterImageRecipes(catalog: RecipeViewModel[]): RecipeViewModel[] {
  return catalog.filter((recipe) => imageRecipeCapability(recipe).genericGenerationCompatible);
}

export function videoRecipeCapability(recipe: RecipeViewModel): VideoRecipeCapability {
  const outputVideo = recipe.outputTypes?.includes("video") ?? false;
  const promptField = findTextarea(recipe, promptKeys);
  const hasFirstFrame = findMediaField(recipe, firstFrameKeys, ["image", "images"]) !== undefined;
  const hasLastFrame = findMediaField(recipe, lastFrameKeys, ["image", "images"]) !== undefined;
  const hasReferenceImage = findMediaField(recipe, referenceImageKeys, ["image", "images"]) !== undefined;
  const hasReferenceAudio = findMediaField(recipe, referenceAudioKeys, ["audio", "audios"]) !== undefined;
  const hasReferenceVideo = findMediaField(recipe, referenceVideoKeys, ["video", "videos"]) !== undefined;
  const requiredMedia = recipe.fields.filter(isMediaField).some((field) => field.required);
  const supportedModes: VideoCompatibleMode[] = [];

  // A Recipe with no required media can be used as a text-to-video workflow,
  // while the presence of an optional reference input remains available to
  // the generic DynamicFormRenderer.
  if (promptField && !requiredMedia) supportedModes.push("FL2VA_TEXT_TO_VIDEO");
  if (promptField && (hasFirstFrame || hasReferenceImage)) supportedModes.push("FL2VA_IMAGE_TO_VIDEO");
  if (promptField && hasFirstFrame && hasLastFrame) supportedModes.push("FL2VA_FIRST_LAST");
  if (promptField && hasReferenceImage) supportedModes.push("REF2VA_IMAGE");
  if (promptField && hasReferenceAudio) supportedModes.push("REF2VA_AUDIO");
  if (promptField && hasReferenceImage && hasReferenceAudio) supportedModes.push("REF2VA_IMAGE_AUDIO");
  if (promptField && hasReferenceVideo && hasReferenceImage) supportedModes.push("REF2VA_VIDEO_IMAGE");

  if (outputVideo) supportedModes.push("CUSTOM_VIDEO");
  const h3Contract = outputVideo ? h3RecipeContract(recipe) : undefined;
  const projectFolderModes = h3Contract?.ok
    ? supportedModes.filter((mode): mode is H3CompatibleMode => mode !== "CUSTOM_VIDEO" && h3ModeSupported(h3Contract.contract, mode))
    : [];

  if (!outputVideo) {
    return {
      outputVideo,
      promptField,
      supportedModes: [],
      projectFolderModes: [],
      genericGenerationCompatible: false,
      reason: "该 Recipe 没有视频输出。",
    };
  }
  return {
    outputVideo,
    promptField,
    supportedModes,
    projectFolderModes,
    genericGenerationCompatible: true,
    reason: promptField
      ? undefined
      : "该工作流没有标准 Prompt 输入，将使用通用参数模式。",
  };
}

export function filterVideoRecipes(catalog: RecipeViewModel[]): RecipeViewModel[] {
  return catalog.filter((recipe) => videoRecipeCapability(recipe).genericGenerationCompatible);
}

export function recipesForVideoMode(
  catalog: RecipeViewModel[],
  mode: H3CompatibleMode | "CUSTOM_VIDEO",
): RecipeViewModel[] {
  return filterVideoRecipes(catalog).filter((recipe) => videoRecipeCapability(recipe).supportedModes.includes(mode));
}

export function resolveVideoRecipe<T extends RecipeViewModel>(
  catalog: T[],
  mode: H3CompatibleMode | "CUSTOM_VIDEO",
  manualSelection?: SelectedRecipeRef,
  recommended?: T,
): ResolvedRecipe<T> {
  const modeCandidates = recipesForVideoMode(catalog, mode) as T[];
  const candidates = modeCandidates.length || mode === "CUSTOM_VIDEO"
    ? modeCandidates
    : recipesForVideoMode(catalog, "CUSTOM_VIDEO") as T[];
  const manual = candidates.find((recipe) => sameRecipeRef(recipe, manualSelection));
  if (manual) return { recipe: manual, source: "manual", staleManualSelection: false };
  const recommendedCandidate = recommended && candidates.some((recipe) => sameRecipeRef(recipe, recommended))
    ? recommended
    : undefined;
  if (recommendedCandidate) {
    return {
      recipe: recommendedCandidate,
      source: "recommended",
      staleManualSelection: Boolean(manualSelection),
    };
  }
  const compatible = candidates[0];
  return {
    recipe: compatible,
    source: compatible ? "compatible" : undefined,
    staleManualSelection: Boolean(manualSelection),
  };
}

export function resolveProjectFolderRecipes(
  catalog: RecipeViewModel[],
  modes: readonly H3CompatibleMode[],
  recommendations: Partial<Record<H3CompatibleMode, SelectedRecipeRef | undefined>>,
  manualOverrides: Partial<Record<H3CompatibleMode, SelectedRecipeRef | undefined>> = {},
): Array<{ mode: H3CompatibleMode; recipe?: RecipeViewModel; source: WorkflowSelectionSource; staleManualSelection: boolean }> {
  return modes.map((mode) => {
    const candidates = recipesForVideoMode(catalog, mode).filter((recipe) => (
      videoRecipeCapability(recipe).projectFolderModes.includes(mode)
    ));
    const manual = candidates.find((recipe) => sameRecipeRef(recipe, manualOverrides[mode]));
    if (manual) return { mode, recipe: manual, source: "manual" as const, staleManualSelection: false };
    const recommended = candidates.find((recipe) => sameRecipeRef(recipe, recommendations[mode]));
    const hasManual = Boolean(manualOverrides[mode]);
    if (recommended) return { mode, recipe: recommended, source: "recommended" as const, staleManualSelection: hasManual };
    return { mode, recipe: candidates[0], source: "compatible" as const, staleManualSelection: hasManual };
  });
}

export function findRecipe(
  catalog: RecipeViewModel[],
  ref: SelectedRecipeRef | undefined,
): RecipeViewModel | undefined {
  return catalog.find((recipe) => sameRecipeRef(recipe, ref));
}

export function migrateGenerationValues(
  previousRecipe: RecipeViewModel | undefined,
  nextRecipe: RecipeViewModel,
  values: GenerationValues,
): GenerationValues {
  const nextValues = defaultValuesForRecipe(nextRecipe);
  if (!previousRecipe) return nextValues;
  const previousFields = new Map(previousRecipe.fields.map((field) => [field.key, field]));
  for (const field of nextRecipe.fields) {
    const previousField = previousFields.get(field.key);
    const value = values[field.key];
    if (!previousField || !value || previousField.type !== field.type || !draftValueMatchesField(field, value)) continue;
    nextValues[field.key] = cloneDraftValue(value);
  }
  return nextValues;
}

export function selectedWorkflowStorageKey(projectId: string, stage: "image" | "video"): string {
  return `aistudio.selectedWorkflow.${projectId}.${stage}`;
}

export function readSelectedRecipeRef(projectId: string, stage: "image" | "video"): SelectedRecipeRef | undefined {
  if (typeof globalThis.localStorage === "undefined") return undefined;
  try {
    const raw = globalThis.localStorage.getItem(selectedWorkflowStorageKey(projectId, stage));
    if (!raw) return undefined;
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed) || typeof parsed.workflowVersionId !== "string" || typeof parsed.recipeId !== "string") return undefined;
    return { workflowVersionId: parsed.workflowVersionId, recipeId: parsed.recipeId };
  } catch {
    return undefined;
  }
}

export function writeSelectedRecipeRef(
  projectId: string,
  stage: "image" | "video",
  ref: SelectedRecipeRef,
): void {
  if (typeof globalThis.localStorage === "undefined") return;
  try {
    globalThis.localStorage.setItem(selectedWorkflowStorageKey(projectId, stage), JSON.stringify(ref));
  } catch {
    // localStorage is a preference only; a restricted WebView must not block generation.
  }
}

export function clearSelectedRecipeRef(projectId: string, stage: "image" | "video"): void {
  if (typeof globalThis.localStorage === "undefined") return;
  try {
    globalThis.localStorage.removeItem(selectedWorkflowStorageKey(projectId, stage));
  } catch {
    // Ignore storage failures.
  }
}

export function projectWorkflowOverridesStorageKey(projectId: string): string {
  return `aistudio.selectedWorkflow.${projectId}.video.byMode`;
}

export function readProjectWorkflowOverrides(
  projectId: string,
): Partial<Record<H3CompatibleMode, SelectedRecipeRef>> {
  if (typeof globalThis.localStorage === "undefined") return {};
  try {
    const raw = globalThis.localStorage.getItem(projectWorkflowOverridesStorageKey(projectId));
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).flatMap(([mode, value]) => {
        if (!isH3Mode(mode) || !isRecord(value) || typeof value.workflowVersionId !== "string" || typeof value.recipeId !== "string") return [];
        return [[mode, { workflowVersionId: value.workflowVersionId, recipeId: value.recipeId }]];
      }),
    ) as Partial<Record<H3CompatibleMode, SelectedRecipeRef>>;
  } catch {
    return {};
  }
}

export function writeProjectWorkflowOverrides(
  projectId: string,
  overrides: Partial<Record<H3CompatibleMode, SelectedRecipeRef>>,
): void {
  if (typeof globalThis.localStorage === "undefined") return;
  try {
    globalThis.localStorage.setItem(projectWorkflowOverridesStorageKey(projectId), JSON.stringify(overrides));
  } catch {
    // Preferences are best effort.
  }
}

function findTextarea(
  recipe: RecipeViewModel,
  keys: ReadonlySet<string>,
): Extract<RecipeField, { type: "textarea" }> | undefined {
  return recipe.fields.find((field): field is Extract<RecipeField, { type: "textarea" }> => (
    field.type === "textarea" && keys.has(field.key)
  ));
}

function findMediaField(
  recipe: RecipeViewModel,
  keys: ReadonlySet<string>,
  types: readonly RecipeField["type"][],
): RecipeField | undefined {
  return recipe.fields.find((field) => keys.has(field.key) && types.includes(field.type));
}

function isMediaField(field: RecipeField): field is Exclude<RecipeField, Extract<RecipeField, { type: "textarea" | "integer" | "seed" }>> {
  return ["image", "images", "video", "videos", "audio", "audios"].includes(field.type);
}

function defaultValuesForRecipe(recipe: RecipeViewModel): GenerationValues {
  const entries: Array<[string, DraftValue | undefined]> = recipe.fields.map((field) => {
    switch (field.type) {
      case "textarea": return [field.key, { type: "string", value: field.default }];
      case "integer": return [field.key, field.default === undefined ? undefined : { type: "integer", value: field.default }];
      case "seed": return [field.key, field.defaultMode === "fixed" ? { type: "seed_fixed", value: field.defaultValue ?? "" } : { type: "seed_random" }];
      case "images": return [field.key, { type: "image_assets", assetIds: [] }];
      case "videos": return [field.key, { type: "video_assets", assetIds: [] }];
      case "audios": return [field.key, { type: "audio_assets", assetIds: [] }];
      case "image":
      case "video":
      case "audio": return [field.key, undefined];
    }
  });
  return Object.fromEntries(
    entries.filter((entry): entry is [string, DraftValue] => entry[1] !== undefined),
  );
}

function draftValueMatchesField(field: RecipeField, value: DraftValue): boolean {
  switch (field.type) {
    case "textarea": return value.type === "string";
    case "integer": return value.type === "integer";
    case "seed": return value.type === "seed_random" || value.type === "seed_fixed";
    case "image": return value.type === "image_asset";
    case "images": return value.type === "image_assets";
    case "video": return value.type === "video_asset";
    case "videos": return value.type === "video_assets";
    case "audio": return value.type === "audio_asset";
    case "audios": return value.type === "audio_assets";
  }
}

function cloneDraftValue(value: DraftValue): DraftValue {
  return JSON.parse(JSON.stringify(value)) as DraftValue;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isH3Mode(value: string): value is H3CompatibleMode {
  return [
    "FL2VA_TEXT_TO_VIDEO",
    "FL2VA_IMAGE_TO_VIDEO",
    "FL2VA_FIRST_LAST",
    "REF2VA_IMAGE",
    "REF2VA_AUDIO",
    "REF2VA_IMAGE_AUDIO",
    "REF2VA_VIDEO_IMAGE",
  ].includes(value as H3CompatibleMode);
}
