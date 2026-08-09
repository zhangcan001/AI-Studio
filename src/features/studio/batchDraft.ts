import type { GenerationValues } from "../../types/generation";

export interface BatchDraftItem {
  id: string;
  workflowName: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}

export function cloneGenerationValues(values: GenerationValues): GenerationValues {
  return JSON.parse(JSON.stringify(values)) as GenerationValues;
}

export function retainFailedBatchItems(
  items: BatchDraftItem[],
  failedIndexes: readonly number[],
): BatchDraftItem[] {
  const failed = new Set(failedIndexes);
  return items.filter((_, index) => failed.has(index));
}
