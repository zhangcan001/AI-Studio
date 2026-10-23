/**
 * H3 / Veda prompts follow the official English writing guide. Dialogue inside
 * <d>…</d> or quotes may stay in its original language. This is a
 * non-blocking hint only; generation is never blocked by language.
 */
export const ENGLISH_PROMPT_HINT = "镜头描述建议用英文书写（对白可保留中文）";

/** Returns the hint when the description (excluding dialogue) is mostly Chinese. */
export function englishPromptHint(prompt: string): string | undefined {
  const text = prompt
    .replace(/<d>[\s\S]*?<\/d>/g, "")
    .replace(/[“"「『][^”"」』]*[”"」』]/g, "");
  const han = (text.match(/\p{Script=Han}/gu) ?? []).length;
  const latin = (text.match(/[A-Za-z]/g) ?? []).length;
  return han >= 4 && han * 2 > latin ? ENGLISH_PROMPT_HINT : undefined;
}

export const LEGACY_PROMPT_FORMAT_HINT = "该批次使用旧版提示词格式，建议重新导入或重建批次";
export const HEAVY_REFERENCE_HINT = "参考图较多且时长较长，可能显存不足";

/** English word count of the description, excluding <d>…</d> dialogue. */
export function englishWordCount(text: string): number {
  return (text.replace(/<d>[\s\S]*?<\/d>/g, "").match(/[A-Za-z][A-Za-z'-]*/g) ?? []).length;
}

export interface H3PromptHintInput {
  description: string;
  durationSeconds?: number;
  referenceCount: number;
  /** Names of bound reference entities that the description never @-mentions. */
  unmentionedNames: string[];
}

/** Non-blocking writing hints for an H3 shot description; never used to block generation. */
export function h3PromptHints({ description, durationSeconds, referenceCount, unmentionedNames }: H3PromptHintInput): string[] {
  const hints: string[] = [];
  const text = description.trim();
  if (text) {
    const language = englishPromptHint(text);
    if (language) hints.push(language);
    const words = englishWordCount(text);
    if (!language && words < 350) hints.push(`镜头描述约 ${words} 个英文单词，建议补充到 350–500 词`);
    const outside = text.replace(/<d>[\s\S]*?<\/d>/g, "");
    if (/[“"「『][^”"」』]*\p{Script=Han}[^”"」』]*[”"」』]/u.test(outside)) hints.push("对白建议写成 <d>[Chinese] 台词。</d> 格式");
    if (durationSeconds !== undefined && Number.isFinite(durationSeconds)) {
      for (const match of text.matchAll(/\[Shot\s+\d+\]\s*At\s+(\d+):(\d+(?:\.\d+)?)/gi)) {
        const at = Number(match[1]) * 60 + Number(match[2]);
        if (at >= durationSeconds) { hints.push(`切镜时间 ${match[1]}:${match[2]} 超出镜头时长 ${durationSeconds} 秒`); break; }
      }
    }
  }
  if (unmentionedNames.length) hints.push(`以下参考未在描述中 @ 到：${unmentionedNames.join("、")}`);
  if (referenceCount >= 7 && (durationSeconds ?? 0) > 10) hints.push(HEAVY_REFERENCE_HINT);
  return hints;
}

/** True when a frozen, already-compiled queue prompt predates the current H3 format. */
export function isLegacyCompiledPrompt(promptText: string | undefined): boolean {
  if (!promptText || !promptText.includes("subject_definitions:")) return false;
  return /opening_composition|ending_composition|composition_reference|Use the referenced subjects for the requested video/.test(promptText)
    || !/\n\[Shot \d+\]/.test(promptText);
}

/** Entity appearance text is injected into the English prompt only when mostly English. */
export function appearanceLanguageHint(description: string): string | undefined {
  return englishPromptHint(description) ? "外貌描述建议用英文书写；非英文描述不会写入生成提示词" : undefined;
}
