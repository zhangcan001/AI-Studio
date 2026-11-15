/**
 * Product-wide shot duration cap. MiniMax H3 / Veda shots longer than this
 * exhaust 16 GB of VRAM, so the UI, folder import and backend admission all
 * reject them (backend: `h3_resolution::MAX_SHOT_DURATION_SECONDS`).
 */
export const MAX_SHOT_DURATION_SECONDS = 11;
export const SHOT_DURATION_LIMIT_MESSAGE = `镜头时长最长 ${MAX_SHOT_DURATION_SECONDS} 秒（显存限制），请改短后再生成。`;

/** Effective input max: the tighter of the Recipe's max and the product cap. */
export function shotDurationMax(recipeMax?: number): number {
  return Math.min(recipeMax ?? MAX_SHOT_DURATION_SECONDS, MAX_SHOT_DURATION_SECONDS);
}

/** Chinese blocker for a duration above the cap; existing shots are never truncated. */
export function shotDurationIssue(seconds: number | undefined): string | undefined {
  return seconds !== undefined && Number.isFinite(seconds) && seconds > MAX_SHOT_DURATION_SECONDS ? SHOT_DURATION_LIMIT_MESSAGE : undefined;
}

/** Validate an editable scalar before persistence, not only on generation. */
export function editableDurationIssue(seconds: number | undefined, field?: { required?: boolean; min?: number; max?: number; type?: string }): string | undefined {
  if (seconds === undefined) return field?.required ? "请填写镜头时长。" : undefined;
  if (!Number.isFinite(seconds)) return "时长必须是有效数字。";
  if (field?.type === "integer" && !Number.isInteger(seconds)) return "时长必须是整数秒。";
  if (shotDurationIssue(seconds)) return SHOT_DURATION_LIMIT_MESSAGE;
  const min = Math.max(1, field?.min ?? 1), max = shotDurationMax(field?.max);
  if (seconds < min) return `此生成器时长最短 ${min} 秒。`;
  if (seconds > max) return max === MAX_SHOT_DURATION_SECONDS ? SHOT_DURATION_LIMIT_MESSAGE : `此生成器时长最长 ${max} 秒。`;
}
