import type { DraftValue, GenerationValues, RecipeViewModel } from "../../types/generation";

export interface ImportedBatchItem {
  workflowName: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}

interface TaskListEnvelope {
  schemaVersion: 1;
  items: unknown[];
}

export function parseBatchTaskList(text: string, catalog: RecipeViewModel[]): ImportedBatchItem[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("BATCH_IMPORT_INVALID_JSON：任务列表不是有效的 JSON。");
  }

  if (!isObject(parsed) || parsed.schemaVersion !== 1 || !Array.isArray(parsed.items)) {
    throw new Error("BATCH_IMPORT_INVALID_SHAPE：任务列表应包含 schemaVersion=1 和 items 数组。");
  }
  const envelope = parsed as unknown as TaskListEnvelope;
  if (envelope.items.length === 0) {
    throw new Error("BATCH_IMPORT_EMPTY：任务列表至少需要一项。");
  }
  if (envelope.items.length > 100) {
    throw new Error("BATCH_IMPORT_TOO_LARGE：任务列表最多包含 100 项。");
  }

  return envelope.items.map((rawItem, index) => {
    if (!isObject(rawItem)) {
      throw new Error(`BATCH_IMPORT_ITEM_INVALID：第 ${index + 1} 项必须是对象。`);
    }
    const workflowVersionId = stringField(rawItem, "workflowVersionId", index);
    const recipeId = stringField(rawItem, "recipeId", index);
    const values = rawItem.values;
    if (!isObject(values) || !isGenerationValues(values)) {
      throw new Error(`BATCH_IMPORT_VALUES_INVALID：第 ${index + 1} 项的参数值无效。`);
    }
    const recipe = catalog.find(
      (candidate) => candidate.workflowVersionId === workflowVersionId && candidate.recipeId === recipeId,
    );
    if (!recipe) {
      throw new Error(`BATCH_IMPORT_WORKFLOW_UNAVAILABLE：第 ${index + 1} 项引用了不可用配方。`);
    }

    return {
      workflowName: recipe.name,
      workflowVersionId,
      recipeId,
      values,
    };
  });
}

function stringField(value: Record<string, unknown>, key: string, index: number): string {
  const field = value[key];
  if (typeof field !== "string" || !field.trim()) {
    throw new Error(`BATCH_IMPORT_ITEM_INVALID：第 ${index + 1} 项缺少 ${key}。`);
  }
  return field;
}

function isGenerationValues(value: Record<string, unknown>): value is GenerationValues {
  return Object.values(value).every(isDraftValue);
}

function isDraftValue(value: unknown): value is DraftValue {
  if (!isObject(value) || typeof value.type !== "string") return false;
  switch (value.type) {
    case "string":
      return typeof value.value === "string";
    case "integer":
      return typeof value.value === "number" && Number.isSafeInteger(value.value);
    case "seed_random":
      return true;
    case "seed_fixed":
      return typeof value.value === "string" && /^\d+$/.test(value.value);
    case "image_asset":
    case "video_asset":
    case "audio_asset":
      return typeof value.assetId === "string" && value.assetId.length > 0;
    case "image_assets":
    case "video_assets":
    case "audio_assets":
      return Array.isArray(value.assetIds) && value.assetIds.every((assetId) => typeof assetId === "string" && assetId.length > 0);
    default:
      return false;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
