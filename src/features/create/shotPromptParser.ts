/**
 * Deterministic, local (no LLM / network) recognition of shot settings typed
 * into a shot prompt: duration, aspect ratio, resolution, story-entity mentions
 * and shot number. The prompt text itself is never rewritten.
 */
import type { StoryEntity } from "../../types/storyEntity";
import type { DraftValue, RecipeField } from "../../types/generation";

export type ParsedResolution = { kind: "lines"; lines: number } | { kind: "size"; width: number; height: number };
export interface ParsedShotPrompt {
  durationSeconds?: number;
  aspect?: string;
  resolution?: ParsedResolution;
  entityIds: string[];
  shotNumber?: number;
  warnings?: string[];
}
export interface ResolutionPresetLike { id: string; label?: string; width: number; height: number }

const ASPECTS = ["16:9", "9:16", "1:1", "4:3", "3:4", "21:9"];
const CN_DIGITS: Record<string, number> = { 零: 0, 一: 1, 二: 2, 两: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9, 十: 10 };

/** 1-99 in Arabic or simple Chinese numerals (五, 十二, 二十). */
function toNumber(raw: string): number | undefined {
  if (/^\d+(\.\d+)?$/.test(raw)) return Number(raw);
  if (!/^[零一二两三四五六七八九十]+$/.test(raw)) return undefined;
  if (raw === "十") return 10;
  const ten = raw.indexOf("十");
  if (ten < 0) return raw.length === 1 ? CN_DIGITS[raw] : undefined;
  const tens = ten === 0 ? 1 : CN_DIGITS[raw.slice(0, ten)];
  const ones = ten === raw.length - 1 ? 0 : CN_DIGITS[raw.slice(ten + 1)];
  return tens === undefined || ones === undefined ? undefined : tens * 10 + ones;
}

const NUM = "(\\d+(?:\\.\\d+)?|[零一二两三四五六七八九十]{1,3})";

export function parseShotPrompt(text: string, entities: readonly StoryEntity[] = []): ParsedShotPrompt {
  const out: ParsedShotPrompt = { entityIds: [] };
  const src = text.normalize("NFKC");
  // Duration: "时长5秒", "5秒", "5s", "5 sec", "8秒钟". Ignore "5s内" style? keep simple, last wins.
  const dur = [...src.matchAll(new RegExp(`(?:时长\\s*[:：]?\\s*)?${NUM}\\s*(?:秒钟?|s(?:ec(?:onds?)?)?)(?![a-z])`, "gi"))];
  for (const m of dur) { const n = toNumber(m[1]); if (n !== undefined && n > 0 && n <= 600) out.durationSeconds = Math.round(n); }
  // Aspect: explicit ratio first, then 竖屏/横屏/方形 words.
  const ratio = [...src.matchAll(/(?<![\d.])(\d{1,2})\s*[:：]\s*(\d{1,2})(?![\d.])/g)].map(m => `${m[1]}:${m[2]}`).filter(r => ASPECTS.includes(r));
  if (ratio.length) out.aspect = ratio[ratio.length - 1];
  else if (/竖屏|竖版|竖构图|portrait/i.test(src)) out.aspect = "9:16";
  else if (/横屏|横版|宽屏|landscape/i.test(src)) out.aspect = "16:9";
  else if (/方形|正方形|方屏|square/i.test(src)) out.aspect = "1:1";
  // Resolution: "1344x768" / "1344×768" / "1344*768", or "720p"/"1080p"/"4k".
  const size = [...src.matchAll(/(?<!\d)(\d{3,4})\s*[x×*]\s*(\d{3,4})(?!\d)/gi)].pop();
  const lines = [...src.matchAll(/(?<!\d)(480|540|720|1080|1440|2160)\s*p(?![a-z])/gi)].pop();
  if (size) out.resolution = { kind: "size", width: Number(size[1]), height: Number(size[2]) };
  else if (lines) out.resolution = { kind: "lines", lines: Number(lines[1]) };
  else if (/(?<![a-z\d])(2k)(?![a-z])/i.test(src)) out.resolution = { kind: "lines", lines: 1440 };
  else if (/(?<![a-z\d])(4k)(?![a-z])/i.test(src)) out.resolution = { kind: "lines", lines: 2160 };
  if (!out.resolution && /(?:画幅|分辨率|尺寸)\s*[:：]?\s*\d{3,4}(?!\d)/.test(src)) {
    out.warnings = ["画幅/分辨率信息不完整：请填写 16:9、9:16 等比例，或 1920×1080 等完整尺寸；单个数字不会自动填入。"];
  }
  // Shot number: "镜头3", "镜号 3", "第3镜", "第三个镜头".
  const shot = [...src.matchAll(new RegExp(`(?:镜头|镜号|分镜)\\s*[:：#]?\\s*${NUM}|第\\s*${NUM}\\s*(?:个)?(?:镜头|镜|分镜)`, "g"))].pop();
  if (shot) { const n = toNumber(shot[1] ?? shot[2]); if (n !== undefined && Number.isInteger(n) && n > 0) out.shotNumber = n; }
  // Entities: "@名称" or exact name occurrences; longest names first so "小明妈妈" beats "小明".
  const sorted = [...entities].filter(e => e.name.trim()).sort((a, b) => b.name.length - a.name.length);
  let rest = src;
  for (const e of sorted) {
    const name = e.name.normalize("NFKC");
    if (rest.includes(name)) { out.entityIds.push(e.id); rest = rest.split(name).join(" "); }
  }
  return out;
}

function aspectOfSize(w: number, h: number) {
  const q = w / h;
  return ASPECTS.find(k => { const [a, b] = k.split(":").map(Number); return Math.abs(a / b - q) < 0.03; });
}
function presetAspect(p: ResolutionPresetLike) {
  const m = /-(\d+)x(\d+)$/.exec(p.id);
  return m ? `${m[1]}:${m[2]}` : aspectOfSize(p.width, p.height);
}

/**
 * Maps a parsed aspect/resolution onto the generator's preset ladder. "720p"
 * means the preset whose pixel count is closest to 1280×720 at the target
 * aspect (default: current aspect, else 16:9). Exact sizes win when a preset
 * matches; otherwise the nearest preset of that aspect is used.
 */
export function resolvePreset(parsed: Pick<ParsedShotPrompt, "aspect" | "resolution">, presets: readonly ResolutionPresetLike[], current?: { width: number; height: number }): ResolutionPresetLike | undefined {
  if (!presets.length || (!parsed.aspect && !parsed.resolution)) return undefined;
  const res = parsed.resolution;
  if (res?.kind === "size") {
    const exact = presets.find(p => p.width === res.width && p.height === res.height);
    if (exact) return exact;
  }
  const aspect = parsed.aspect ?? (res?.kind === "size" ? aspectOfSize(res.width, res.height) : undefined) ?? (current ? aspectOfSize(current.width, current.height) : undefined) ?? "16:9";
  const pool = presets.filter(p => presetAspect(p) === aspect);
  if (!pool.length) return undefined;
  let target: number;
  if (res?.kind === "size") target = res.width * res.height;
  else if (res?.kind === "lines") target = res.lines * res.lines * 16 / 9;
  else if (current) target = current.width * current.height;
  else target = 1280 * 720;
  return [...pool].sort((a, b) => Math.abs(a.width * a.height - target) - Math.abs(b.width * b.height - target))[0];
}

/** Project only explicit numeric settings onto the selected Recipe's real constraints. */
export function resolvePromptSettings(parsed: ParsedShotPrompt, generator: { fields: readonly RecipeField[]; resolutionPresets?: readonly ResolutionPresetLike[] } | undefined, current: Record<string, DraftValue>) {
  const warnings = [...(parsed.warnings ?? [])];
  let duration: { type: "integer" | "number"; value: number } | undefined;
  if (parsed.durationSeconds !== undefined) {
    const field = generator?.fields.find(f => f.key === "duration_seconds");
    if (field && (field.type === "integer" || field.type === "number")) {
      duration = { type: field.type, value: Math.min(field.max ?? Infinity, Math.max(field.min ?? -Infinity, parsed.durationSeconds)) };
    } else warnings.push("当前生成器没有时长参数，未自动填入。");
  }
  const width = current.width, height = current.height;
  const size = (width?.type === "integer" || width?.type === "number") && (height?.type === "integer" || height?.type === "number") ? { width: width.value, height: height.value } : undefined;
  const presets = generator?.resolutionPresets ?? [];
  const preset = resolvePreset(parsed, presets, size);
  if ((parsed.aspect || parsed.resolution) && !preset) {
    warnings.push(presets.length ? "当前生成器没有匹配的画幅/分辨率预设，未自动填入；请从可用预设中选择。" : "请先选择支持画幅/分辨率预设的生成器，未自动填入。");
  }
  return { duration, preset, warnings };
}
