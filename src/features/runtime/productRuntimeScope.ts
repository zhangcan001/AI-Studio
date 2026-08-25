import type { RecipeField, RecipeViewModel } from "../../types/generation";

export const KERA2_WORKFLOW_ID = "wfl_kera2_t2i_local_v2" as const;
export const MINIMAX_H3_WORKFLOW_ID = "wfl_minimax_h3_reference_video" as const;
export const MINIMAX_H3_FL2VA_WORKFLOW_ID = "wfl_minimax_h3_fl2va" as const;
export const MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID = "wfl_minimax_h3_fl2va_t2v_quality" as const;
export const MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID = "wfl_minimax_h3_fl2va_i2v_quality" as const;
export const MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID = "wfl_minimax_h3_fl2va_first_last_quality" as const;
export const MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID = "wfl_minimax_h3_reference_video_quality" as const;
export const MINIMAX_H3_FAST_WORKFLOW_IDS = [MINIMAX_H3_WORKFLOW_ID, MINIMAX_H3_FL2VA_WORKFLOW_ID] as const;
export const MINIMAX_H3_QUALITY_WORKFLOW_IDS = [
  MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
] as const;
export const MINIMAX_H3_WORKFLOW_IDS = [...MINIMAX_H3_FAST_WORKFLOW_IDS, ...MINIMAX_H3_QUALITY_WORKFLOW_IDS] as const;
export const PRODUCTION_WORKFLOW_IDS = [KERA2_WORKFLOW_ID, ...MINIMAX_H3_WORKFLOW_IDS] as const;

export type H3QualityProfile = "QUALITY" | "FAST";

export const H3_QUALITY_PROFILE: H3QualityProfile = "QUALITY";
export const H3_FAST_PROFILE: H3QualityProfile = "FAST";

export type ProductionRuntimeKind = "kera2Image" | "minimaxH3Video";
export type ProductionShotStage = "image" | "video";

export function productionRuntimeForWorkflowId(workflowId: string): ProductionRuntimeKind | undefined {
  switch (workflowId) {
    case KERA2_WORKFLOW_ID:
      return "kera2Image";
    case MINIMAX_H3_WORKFLOW_ID:
    case MINIMAX_H3_FL2VA_WORKFLOW_ID:
    case MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID:
    case MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID:
    case MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID:
    case MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID:
      return "minimaxH3Video";
    default:
      return undefined;
  }
}

export function h3FamilyForWorkflowId(workflowId: string): "FL2VA" | "REF2VA" | undefined {
  if (
    workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID
    || workflowId === MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID
    || workflowId === MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID
    || workflowId === MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID
  ) return "FL2VA";
  if (workflowId === MINIMAX_H3_WORKFLOW_ID || workflowId === MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID) return "REF2VA";
  return undefined;
}

export function h3QualityProfileForWorkflowId(workflowId: string): H3QualityProfile | undefined {
  if (MINIMAX_H3_QUALITY_WORKFLOW_IDS.includes(workflowId as typeof MINIMAX_H3_QUALITY_WORKFLOW_IDS[number])) return H3_QUALITY_PROFILE;
  if (MINIMAX_H3_FAST_WORKFLOW_IDS.includes(workflowId as typeof MINIMAX_H3_FAST_WORKFLOW_IDS[number])) return H3_FAST_PROFILE;
  return undefined;
}

export function h3WorkflowIdForMode(mode: string, profile: H3QualityProfile): string | undefined {
  if (profile === H3_FAST_PROFILE) {
    return mode.startsWith("FL2VA") ? MINIMAX_H3_FL2VA_WORKFLOW_ID : MINIMAX_H3_WORKFLOW_ID;
  }
  switch (mode) {
    case "FL2VA_TEXT_TO_VIDEO": return MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID;
    case "FL2VA_IMAGE_TO_VIDEO": return MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID;
    case "FL2VA_FIRST_LAST": return MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID;
    case "REF2VA_IMAGE":
    case "REF2VA_AUDIO":
    case "REF2VA_IMAGE_AUDIO":
    case "REF2VA_VIDEO_IMAGE": return MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID;
    default: return undefined;
  }
}

export function h3RecipeForMode<T extends { workflowId: string; outputTypes?: string[] }>(
  catalog: T[],
  mode: string,
  profile: H3QualityProfile,
): T | undefined {
  const workflowId = h3WorkflowIdForMode(mode, profile);
  return workflowId
    ? catalog.find((recipe) => recipe.workflowId === workflowId && recipe.outputTypes?.includes("video"))
    : undefined;
}

export function productionRuntimeForStage(
  stage: ProductionShotStage,
  workflowId: string,
): ProductionRuntimeKind | undefined {
  const runtime = productionRuntimeForWorkflowId(workflowId);
  if (stage === "image" && runtime === "kera2Image") return runtime;
  if (stage === "video" && runtime === "minimaxH3Video") return runtime;
  return undefined;
}

export function isProductionRuntimeForStage(stage: ProductionShotStage, workflowId: string): boolean {
  return productionRuntimeForStage(stage, workflowId) !== undefined;
}

export function filterProductionRuntimeCatalog(catalog: RecipeViewModel[]): RecipeViewModel[] {
  return catalog.filter((recipe) => productionRuntimeForWorkflowId(recipe.workflowId) !== undefined);
}

export function kera2PromptField(
  recipe: RecipeViewModel,
): Extract<RecipeField, { type: "textarea" }> | undefined {
  if (recipe.workflowId !== KERA2_WORKFLOW_ID) return undefined;
  return recipe.fields.find((field) => field.key === "prompt" && field.type === "textarea") as
    | Extract<RecipeField, { type: "textarea" }>
    | undefined;
}

export type Kera2RecipeContractResult =
  | {
      ok: true;
      contract: {
        promptField: Extract<RecipeField, { type: "textarea" }>;
        widthField: Extract<RecipeField, { type: "integer" }>;
        heightField: Extract<RecipeField, { type: "integer" }>;
        seedField: Extract<RecipeField, { type: "seed" }>;
      };
    }
  | { ok: false; reason: string };

export function kera2RecipeContract(recipe: RecipeViewModel): Kera2RecipeContractResult {
  if (recipe.workflowId !== KERA2_WORKFLOW_ID) {
    return { ok: false, reason: "运行目录中的配方不是 Krea2。" };
  }
  const promptField = recipe.fields.find((field) => field.key === "prompt" && field.type === "textarea") as
    | Extract<RecipeField, { type: "textarea" }>
    | undefined;
  if (!promptField) return { ok: false, reason: "Krea2 配方缺少键为 `prompt` 的文本输入字段。" };
  const widthField = recipe.fields.find((field) => field.key === "width" && field.type === "integer") as
    | Extract<RecipeField, { type: "integer" }>
    | undefined;
  if (!widthField) return { ok: false, reason: "Krea2 配方缺少键为 `width` 的整数输入字段。" };
  const heightField = recipe.fields.find((field) => field.key === "height" && field.type === "integer") as
    | Extract<RecipeField, { type: "integer" }>
    | undefined;
  if (!heightField) return { ok: false, reason: "Krea2 配方缺少键为 `height` 的整数输入字段。" };
  const seedField = recipe.fields.find((field) => field.key === "seed" && field.type === "seed") as
    | Extract<RecipeField, { type: "seed" }>
    | undefined;
  if (!seedField) return { ok: false, reason: "Krea2 配方缺少键为 `seed` 的 Seed 输入字段。" };
  if (!recipe.outputTypes?.includes("image")) {
    return { ok: false, reason: "Krea2 配方未声明图片输出。" };
  }
  if (!promptField.required || !widthField.required || !heightField.required) {
    return { ok: false, reason: "Krea2 配方的 prompt、width、height 必须是必填字段。" };
  }
  return { ok: true, contract: { promptField, widthField, heightField, seedField } };
}
