import type {
  DraftValue,
  GenerationValues,
  RecipeField,
  RecipeViewModel,
  StudioAssetType,
} from "../../types/generation";

export type AssetIntentAssignment =
  | { kind: "applied"; values: GenerationValues }
  | { kind: "requires_confirmation" }
  | { kind: "max_items" }
  | { kind: "incompatible" };

export function compatibleAssetFields(
  recipe: RecipeViewModel,
  assetType: StudioAssetType,
): RecipeField[] {
  return recipe.fields.filter((field) => {
    if (assetType === "image") return field.type === "image" || field.type === "images";
    if (assetType === "video") return field.type === "video" || field.type === "videos";
    return field.type === "audio" || field.type === "audios";
  });
}

export function assignAssetToField(
  field: RecipeField,
  values: GenerationValues,
  assetId: string,
  replaceSingle = false,
): AssetIntentAssignment {
  const next = { ...values };
  if (field.type === "image" || field.type === "video" || field.type === "audio") {
    const current = values[field.key];
    if (current && !replaceSingle) return { kind: "requires_confirmation" };
    next[field.key] = singleAssetValue(field.type, assetId);
    return { kind: "applied", values: next };
  }

  if (field.type !== "images" && field.type !== "videos" && field.type !== "audios") {
    return { kind: "incompatible" };
  }
  const currentIds = listAssetIds(values[field.key], field.type);
  if (currentIds.includes(assetId)) return { kind: "applied", values: next };
  if (currentIds.length >= field.maxItems) return { kind: "max_items" };
  next[field.key] = listAssetValue(field.type, [...currentIds, assetId]);
  return { kind: "applied", values: next };
}

function singleAssetValue(
  fieldType: "image" | "video" | "audio",
  assetId: string,
): DraftValue {
  return { type: `${fieldType}_asset` as "image_asset" | "video_asset" | "audio_asset", assetId };
}

function listAssetIds(
  value: DraftValue | undefined,
  fieldType: "images" | "videos" | "audios",
): string[] {
  if (fieldType === "images" && value?.type === "image_assets") return [...value.assetIds];
  if (fieldType === "videos" && value?.type === "video_assets") return [...value.assetIds];
  if (fieldType === "audios" && value?.type === "audio_assets") return [...value.assetIds];
  return [];
}

function listAssetValue(
  fieldType: "images" | "videos" | "audios",
  assetIds: string[],
): DraftValue {
  return {
    type: `${fieldType.slice(0, -1)}_assets` as "image_assets" | "video_assets" | "audio_assets",
    assetIds,
  };
}
