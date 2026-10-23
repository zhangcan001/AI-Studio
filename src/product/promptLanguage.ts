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
