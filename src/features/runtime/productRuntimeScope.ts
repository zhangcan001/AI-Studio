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
