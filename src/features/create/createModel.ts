import type { AppRoute } from "../../app/routes/types";
import type { CreationAsset, CreationShot, GeneratorOption, ProductRun } from "../../product/types";
import type { DraftValue, GenerationValues, RecipeField } from "../../types/generation";
import { defaultGenerationValues } from "../../stores/studioStore";
export type CreateRoute = Extract<AppRoute, { kind: "create" }>;
export const scopeKey = (route: CreateRoute) => `${route.projectId}:${route.shotId ?? ""}:${route.stage}`;
export function normalCreate(route: AppRoute) { return route.kind === "create" && route.surface !== "batch"; }
export function modeLabel(generator: GeneratorOption): string {
  if (generator.mediaKind === "image") return generator.fields.some(f => f.type === "image" || f.type === "images") ? "参考图生图" : "文生图";
  if (generator.fields.some(f => f.type === "video" || f.type === "videos")) return "参考视频";
  if (generator.fields.some(f => f.key === "first_frame") && generator.fields.some(f => f.key === "last_frame")) return "首尾帧";
  if (generator.fields.some(f => f.type === "image" || f.type === "images")) return "图生视频";
  return "文生视频";
}
export function generatorLabel(generator: GeneratorOption, index: number) {
  const technical = /(?:^wfl_|^wfv_|\.json$|\.yaml$|[\\/]|[0-9a-f]{32})/i.test(generator.name);
  const name = technical ? /h3|minimax/i.test(generator.name) ? "H3 高质量" : /krea/i.test(generator.name) ? "Krea 图片" : `生成器 ${index + 1}` : generator.name;
  return `${name} · ${modeLabel(generator)} · 版本 ${generator.version}`;
}
export function fieldLabel(field: RecipeField) {
  return ({ prompt: "提示词", negative_prompt: "负向提示词", first_frame: "首帧", last_frame: "尾帧", reference_video: "参考视频", reference_videos: "参考视频", duration_seconds: "时长（秒）", width: "宽度", height: "高度" } as Record<string, string>)[field.key] ?? field.label;
}
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
  return values;
}
function compatible(field: RecipeField, value: DraftValue) {
  const expected = ({ textarea: "string", integer: "integer", number: "number", image: "image_asset", images: "image_assets", video: "video_asset", videos: "video_assets", audio: "audio_asset", audios: "audio_assets" } as Record<string, string>)[field.type];
  return field.type === "seed" ? value.type === "seed_random" || value.type === "seed_fixed" : value.type === expected;
}
export const runLabels: Record<ProductRun["status"], string> = { QUEUED: "排队中", RUNNING: "运行中", PAUSED: "暂停", FAILED: "失败", PARTIAL: "部分完成", SUCCEEDED: "已完成", CANCELLED: "已取消" };
export function candidateForStage(asset: CreationAsset, route: CreateRoute) { return asset.mediaKind === route.stage; }
