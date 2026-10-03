import type { GeneratorOption } from "./types";
import type { RecipeField } from "../types/generation";

// Shared presentation vocabulary used by creation and project generator settings.
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
