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

export function withBatchPrompt(
  item: BatchDraftItem,
  promptFieldKey: string,
  promptText: string,
): BatchDraftItem {
  return {
    ...item,
    values: {
      ...cloneGenerationValues(item.values),
      [promptFieldKey]: { type: "string", value: promptText },
    },
  };
}

export function copyBatchDraftItem(
  items: BatchDraftItem[],
  id: string,
  copiedId: string,
): BatchDraftItem[] {
  const sourceIndex = items.findIndex((item) => item.id === id);
  if (sourceIndex < 0) return items;
  const source = items[sourceIndex];
  const copy = { ...source, id: copiedId, values: cloneGenerationValues(source.values) };
  return [...items.slice(0, sourceIndex + 1), copy, ...items.slice(sourceIndex + 1)];
}

export function moveBatchDraftItem(
  items: BatchDraftItem[],
  id: string,
  direction: -1 | 1,
): BatchDraftItem[] {
  const index = items.findIndex((item) => item.id === id);
  const nextIndex = index + direction;
  if (index < 0 || nextIndex < 0 || nextIndex >= items.length) return items;
  const next = [...items];
  [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
  return next;
}

export function removeBatchDraftItem(items: BatchDraftItem[], id: string): BatchDraftItem[] {
  return items.filter((item) => item.id !== id);
}

export function retainFailedBatchItems(
  items: BatchDraftItem[],
  failedIndexes: readonly number[],
): BatchDraftItem[] {
  const failed = new Set(failedIndexes);
  return items.filter((_, index) => failed.has(index));
}
