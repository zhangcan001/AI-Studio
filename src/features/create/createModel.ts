import type { AppRoute } from "../../app/routes/types";
import type { CreationAsset, CreationShot, GeneratorOption, ProductRun } from "../../product/types";
import type { DraftValue, GenerationValues, RecipeField } from "../../types/generation";
import { defaultGenerationValues } from "../../stores/studioStore";
export type CreateRoute = Extract<AppRoute, { kind: "create" }>;
export const scopeKey = (route: CreateRoute) => `${route.projectId}:${route.shotId ?? ""}:${route.stage}`;
export function normalCreate(route: AppRoute) { return route.kind === "create" && route.surface !== "batch"; }
export { modeLabel, generatorLabel, fieldLabel } from "../../product/generatorPresentation";
export function mediaKind(field: RecipeField): CreationAsset["mediaKind"] | null {
  if (field.type === "image" || field.type === "images") return "image";
  if (field.type === "video" || field.type === "videos") return "video";
  if (field.type === "audio" || field.type === "audios") return "audio";
  return null;
}
export function mediaValue(field: RecipeField, ids: string[]): DraftValue {
  switch (field.type) {
    case "image": return { type: "image_asset", assetId: ids[0] };
    case "images": return { type: "image_assets", assetIds: ids };
    case "video": return { type: "video_asset", assetId: ids[0] };
    case "videos": return { type: "video_assets", assetIds: ids };
    case "audio": return { type: "audio_asset", assetId: ids[0] };
    case "audios": return { type: "audio_assets", assetIds: ids };
    default: throw new Error("Not a media field");
  }
}
export function assetIds(value?: DraftValue): string[] { return value && "assetIds" in value ? value.assetIds : value && "assetId" in value ? [value.assetId] : []; }
export function draftFor(generator: GeneratorOption, shot: CreationShot | null, previous: GenerationValues = {}) {
  const values = defaultGenerationValues(generator);
  for (const field of generator.fields) {
    const value = previous[field.key] ?? (shot?.selectionRef === generator.selectionRef ? shot.values[field.key] : undefined);
    if (value && compatible(field, value)) values[field.key] = value;
  }
  if (generator.fields.some(field => field.key === "prompt" && field.type === "textarea") && !previous.prompt) values.prompt = { type: "string", value: shot?.prompt ?? "" };
  const native = generator.resolutionPresets?.[0];
  // Only a new draft gets a native default. Existing/reused snapshots remain
  // untouched until the user explicitly chooses another resolution.
  if (native && !previous.width && !previous.height && !(shot?.selectionRef === generator.selectionRef && (shot.values.width || shot.values.height))) {
    values.width = { type: "integer", value: native.width };
    values.height = { type: "integer", value: native.height };
  }
  return values;
}
function compatible(field: RecipeField, value: DraftValue) {
  const expected = ({ textarea: "string", integer: "integer", number: "number", image: "image_asset", images: "image_assets", video: "video_asset", videos: "video_assets", audio: "audio_asset", audios: "audio_assets" } as Record<string, string>)[field.type];
  return field.type === "seed" ? value.type === "seed_random" || value.type === "seed_fixed" : value.type === expected;
}
export const runLabels: Record<ProductRun["status"], string> = { QUEUED: "排队中", RUNNING: "运行中", PAUSED: "暂停", FAILED: "失败", PARTIAL: "部分完成", SUCCEEDED: "已完成", CANCELLED: "已取消" };
export function candidateForStage(asset: CreationAsset, route: CreateRoute) { return asset.mediaKind === route.stage; }
