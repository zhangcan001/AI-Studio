import type { GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";

export const MAX_WORKFLOW_EXECUTION_BATCH_ITEMS = 100;

export interface MaterializedWorkflowExecutionBatch {
  items: Array<{ values: GenerationValues }>;
  expectedCount: number;
  errors: string[];
}

export function parseBatchPrompts(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

export function materializeWorkflowExecutionBatch(
  recipe: RecipeViewModel,
  baseValues: GenerationValues,
  promptLines: string,
  seedCount: number,
  imageAssetIds: string[],
): MaterializedWorkflowExecutionBatch {
  const errors: string[] = [];
  const promptField = recipe.fields.find((field) => field.type === "textarea" && /prompt/i.test(field.key));
  const seedField = recipe.fields.find((field) => field.type === "seed");
  const imageField = recipe.fields.find((field) => (
    field.type === "image" || (field.type === "images" && field.minItems <= 1 && field.maxItems >= 1)
  ));
  const prompts = promptField && parseBatchPrompts(promptLines).length
    ? parseBatchPrompts(promptLines)
    : [undefined];
  const images = [...new Set(imageAssetIds)];

  if (!Number.isInteger(seedCount) || seedCount < 1) {
    errors.push("BATCH_SEED_COUNT_INVALID");
  }
  if (seedCount > MAX_WORKFLOW_EXECUTION_BATCH_ITEMS) {
    errors.push("BATCH_TOO_LARGE");
  }
  if (seedCount > 1 && !seedField) {
    errors.push("SEED_BATCH_UNSUPPORTED");
  }
  if (images.length && !imageField) {
    errors.push("IMAGE_BATCH_UNSUPPORTED");
  }
  if (images.length > 1 && (prompts.length > 1 || seedCount > 1)) {
    errors.push("IMAGE_BATCH_REQUIRES_ONE_PROMPT_AND_ONE_SEED");
  }

  const imageChoices: Array<string | undefined> = images.length ? images : [undefined];
  const expectedCount = prompts.length * seedCount * imageChoices.length;
  if (expectedCount > MAX_WORKFLOW_EXECUTION_BATCH_ITEMS) {
    errors.push("BATCH_TOO_LARGE");
  }
  if (errors.length) return { items: [], expectedCount, errors: [...new Set(errors)] };

  const items: Array<{ values: GenerationValues }> = [];
  for (const assetId of imageChoices) {
    for (const prompt of prompts) {
      for (let seedIndex = 0; seedIndex < seedCount; seedIndex += 1) {
        const values = { ...baseValues };
        if (promptField && prompt !== undefined) {
          values[promptField.key] = { type: "string", value: prompt };
        }
        if (assetId && imageField) {
          values[imageField.key] = imageField.type === "images"
            ? { type: "image_assets", assetIds: [assetId] }
            : { type: "image_asset", assetId };
        }
        if (seedField && seedCount > 1) {
          const seedValue = baseValues[seedField.key];
          if (seedValue?.type === "seed_fixed") {
            const next = incrementSeed(seedValue.value, seedIndex);
            if (!next) {
              errors.push("BATCH_SEED_RANGE_EXCEEDED");
              continue;
            }
            values[seedField.key] = { type: "seed_fixed", value: next };
          } else {
            // ProductionQueueService resolves each random value before persisting
            // the batch item, then stores the fixed seed for restart-safe replay.
            values[seedField.key] = { type: "seed_random" };
          }
        }
        items.push({ values });
      }
    }
  }
  return { items, expectedCount, errors: [...new Set(errors)] };
}

export function workflowExecutionBatchFields(recipe: RecipeViewModel): {
  prompt?: Extract<RecipeField, { type: "textarea" }>;
  seed?: Extract<RecipeField, { type: "seed" }>;
  image?: Extract<RecipeField, { type: "image" | "images" }>;
} {
  return {
    prompt: recipe.fields.find((field): field is Extract<RecipeField, { type: "textarea" }> => (
      field.type === "textarea" && /prompt/i.test(field.key)
    )),
    seed: recipe.fields.find((field): field is Extract<RecipeField, { type: "seed" }> => field.type === "seed"),
    image: recipe.fields.find((field): field is Extract<RecipeField, { type: "image" | "images" }> => (
      field.type === "image" || (field.type === "images" && field.minItems <= 1 && field.maxItems >= 1)
    )),
  };
}

function incrementSeed(value: string, offset: number): string | undefined {
  try {
    const next = BigInt(value) + BigInt(offset);
    if (next > 18_446_744_073_709_551_615n) return undefined;
    return next.toString();
  } catch {
    return undefined;
  }
}
