import type { AssetView } from "../../types/asset";
import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";

export function isImageAssetForVideo(asset: AssetView): boolean {
  return asset.assetType === "image" || asset.category === "source_image" || asset.category === "generated_image";
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
): GenerationValues {
  const values: GenerationValues = {};
  for (const field of recipe.fields) {
    const value = defaultValueForField(field);
    if (value) values[field.key] = value;
  }
  const promptField = recipe.fields.find((field) => field.type === "textarea");
  if (promptField) values[promptField.key] = { type: "string", value: promptText.trim() };
  const imageField = recipe.fields.find((field) => field.type === "image" || field.type === "images");
  if (imageField) {
    values[imageField.key] = imageField.type === "images"
      ? { type: "image_assets", assetIds: [assetId] }
      : { type: "image_asset", assetId };
  }
  return values;
}

export function h3PromptField(recipe: RecipeViewModel): RecipeField | undefined {
  return recipe.fields.find((field) => field.type === "textarea");
}

export function h3ReferenceField(recipe: RecipeViewModel): RecipeField | undefined {
  return recipe.fields.find((field) => field.type === "image" || field.type === "images");
}

function defaultValueForField(field: RecipeField): DraftValue | undefined {
  switch (field.type) {
    case "textarea":
      return { type: "string", value: field.default };
    case "integer":
      return { type: "integer", value: safeH3Integer(field) };
    case "seed":
      return { type: "seed_random" };
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

function safeH3Integer(field: Extract<RecipeField, { type: "integer" }>): number {
  const key = field.key.toLowerCase();
  const requested = key.includes("step") ? 4 : key.includes("duration") || key.includes("length") || key.includes("second") ? 1 : field.default ?? field.min ?? 1;
  return Math.min(field.max ?? requested, Math.max(field.min ?? requested, requested));
}
