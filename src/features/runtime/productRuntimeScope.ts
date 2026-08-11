import type { RecipeField, RecipeViewModel } from "../../types/generation";

export const KERA2_WORKFLOW_ID = "wfl_kera2_t2i_local_v2" as const;
export const MINIMAX_H3_WORKFLOW_ID = "wfl_minimax_h3_reference_video" as const;
export const PRODUCTION_WORKFLOW_IDS = [KERA2_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID] as const;

export type ProductionRuntimeKind = "kera2Image" | "minimaxH3Video";
export type ProductionShotStage = "image" | "video";

export function productionRuntimeForWorkflowId(workflowId: string): ProductionRuntimeKind | undefined {
  switch (workflowId) {
    case KERA2_WORKFLOW_ID:
      return "kera2Image";
    case MINIMAX_H3_WORKFLOW_ID:
      return "minimaxH3Video";
    default:
      return undefined;
  }
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
    return { ok: false, reason: "运行目录中的 Recipe 不是 Krea2。" };
  }
  const promptField = recipe.fields.find((field) => field.key === "prompt" && field.type === "textarea") as
    | Extract<RecipeField, { type: "textarea" }>
    | undefined;
  if (!promptField) return { ok: false, reason: "Krea2 Recipe 缺少 key 为 `prompt` 的 textarea 字段。" };
  const widthField = recipe.fields.find((field) => field.key === "width" && field.type === "integer") as
    | Extract<RecipeField, { type: "integer" }>
    | undefined;
  if (!widthField) return { ok: false, reason: "Krea2 Recipe 缺少 key 为 `width` 的 integer 字段。" };
  const heightField = recipe.fields.find((field) => field.key === "height" && field.type === "integer") as
    | Extract<RecipeField, { type: "integer" }>
    | undefined;
  if (!heightField) return { ok: false, reason: "Krea2 Recipe 缺少 key 为 `height` 的 integer 字段。" };
  const seedField = recipe.fields.find((field) => field.key === "seed" && field.type === "seed") as
    | Extract<RecipeField, { type: "seed" }>
    | undefined;
  if (!seedField) return { ok: false, reason: "Krea2 Recipe 缺少 key 为 `seed` 的 seed 字段。" };
  if (!recipe.outputTypes?.includes("image")) {
    return { ok: false, reason: "Krea2 Recipe 未声明图片输出。" };
  }
  if (!promptField.required || !widthField.required || !heightField.required) {
    return { ok: false, reason: "Krea2 Recipe 的 prompt、width、height 必须是必填字段。" };
  }
  return { ok: true, contract: { promptField, widthField, heightField, seedField } };
}
