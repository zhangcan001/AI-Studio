import type { RecipeViewModel } from "../../types/generation";
import type { ShotView } from "../../types/shot";
import type { VideoInputAsset, VideoInputKey } from "../../types/shotVideoInput";

export const BATCH_BIND_KEYS = ["first_frame","last_frame","reference_images","reference_videos","reference_audios"] as const;
export type BatchBindKey = (typeof BATCH_BIND_KEYS)[number];
export const BATCH_BIND_KEY_LABELS: Record<BatchBindKey, string> = {
  first_frame: "首帧", last_frame: "尾帧", reference_images: "参考图片",
  reference_videos: "参考视频", reference_audios: "参考音频",
};
const PLURAL = new Set<BatchBindKey>(["reference_images","reference_videos","reference_audios"]);
const SLOT_MAX: Partial<Record<BatchBindKey, number>> = { reference_images: 9, reference_videos: 3, reference_audios: 3 };

export function encodeExactSelectionRef(workflowVersionId: string, recipeId: string): string {
  if (!workflowVersionId.trim() || !recipeId.trim()) throw new Error("生成器身份不完整，无法绑定正式输入槽。");
  const bytes = new TextEncoder().encode(JSON.stringify([workflowVersionId, recipeId]));
  return "generator:v1:" + Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
export function isBatchBindKey(value: string): value is BatchBindKey {
  return (BATCH_BIND_KEYS as readonly string[]).includes(value);
}
export function recipeMediaSlots(recipe: RecipeViewModel): BatchBindKey[] {
  return recipe.fields.map((f) => f.key).filter(isBatchBindKey);
}
export function videoStageConfig(shot: ShotView) {
  return shot.stageConfigs.find((c) => c.stage === "video");
}
export function recipeForShot(shot: ShotView, catalog: RecipeViewModel[]) {
  const config = videoStageConfig(shot);
  if (!config) return undefined;
  return catalog.find((r) => r.workflowVersionId === config.workflowVersionId && r.recipeId === config.recipeId);
}
export function assetAllowedForBindKey(inputKey: BatchBindKey, category: string, sourceTaskId?: string | null): boolean {
  const normalized = (category ?? "").trim();
  if (!normalized) return false;
  const generated = !!sourceTaskId || normalized.startsWith("generated_");
  if (inputKey === "first_frame" || inputKey === "last_frame" || inputKey === "reference_images") {
    return !generated && normalized === "source_image";
  }
  if (inputKey === "reference_videos") return !generated && normalized === "source_video";
  if (inputKey === "reference_audios") return !generated && normalized === "source_audio";
  return false;
}

export interface ParsedBindRow {
  line: number; shotOrdinal?: number; shotId?: string; inputKey?: string; assetId?: string; errors: string[];
}
export function parseBatchBindTable(content: string): ParsedBindRow[] {
  const lines = content.replace(/^\uFEFF/, "").split(/\r?\n/).map((l) => l.trimEnd());
  const rows: ParsedBindRow[] = [];
  let started = false;
  for (let index = 0; index < lines.length; index += 1) {
    const raw = lines[index];
    if (!raw.trim()) continue;
    const cells = (raw.includes("\t") ? raw.split("\t") : raw.split(",")).map((c) => c.trim());
    const [shotCell = "", keyCell = "", assetCell = ""] = cells;
    if (!started && /^(shot_ordinal|shot_id|镜头序号|镜头id)$/i.test(shotCell) && /^(input_key|槽位|inputKey)$/i.test(keyCell)) {
      started = true; continue;
    }
    started = true;
    const errors: string[] = [];
    let shotOrdinal: number | undefined;
    let shotId: string | undefined;
    if (!shotCell) errors.push("缺少镜头序号或镜头 ID");
    else if (/^\d+$/.test(shotCell)) {
      shotOrdinal = Number(shotCell);
      if (shotOrdinal < 1) errors.push("镜头序号必须从 1 开始");
    } else shotId = shotCell;
    if (!keyCell) errors.push("缺少 input_key");
    else if (!isBatchBindKey(keyCell)) errors.push(`不支持的槽位 ${keyCell}；仅允许 ${BATCH_BIND_KEYS.join(" / ")}`);
    if (!assetCell) errors.push("缺少 asset_id");
    rows.push({ line: index + 1, shotOrdinal, shotId, inputKey: keyCell, assetId: assetCell, errors });
  }
  return rows;
}

export interface ResolvedBindAssignment {
  line: number; shotId: string; shotName: string; shotOrdinal: number; inputKey: BatchBindKey;
  assetId: string; workflowVersionId: string; recipeId: string; selectionRef: string; errors: string[];
}
export function resolveBatchBindRows(rows: ParsedBindRow[], shots: ShotView[], catalog: RecipeViewModel[]): ResolvedBindAssignment[] {
  const byId = new Map(shots.map((s) => [s.id, s]));
  const byOrdinal = new Map(shots.map((s) => [s.ordinal + 1, s] as const));
  return rows.map((row) => {
    const errors = [...row.errors];
    const shot = (row.shotId ? byId.get(row.shotId) : undefined) ?? (row.shotOrdinal !== undefined ? byOrdinal.get(row.shotOrdinal) : undefined);
    if (!shot) errors.push("项目中找不到该镜头");
    const config = shot ? videoStageConfig(shot) : undefined;
    if (shot && !config) errors.push("镜头尚未配置视频阶段配方");
    const recipe = shot && config ? catalog.find((item) => item.workflowVersionId === config.workflowVersionId && item.recipeId === config.recipeId) : undefined;
    if (shot && config && !recipe) errors.push("镜头视频配方不在当前目录中");
    const inputKey = row.inputKey && isBatchBindKey(row.inputKey) ? row.inputKey : undefined;
    if (recipe && inputKey && !recipeMediaSlots(recipe).includes(inputKey)) errors.push(`当前配方没有槽位 ${inputKey}`);
    const workflowVersionId = config?.workflowVersionId ?? "";
    const recipeId = config?.recipeId ?? "";
    let selectionRef = "";
    try {
      if (workflowVersionId && recipeId) selectionRef = recipe?.selectionRef?.trim() || encodeExactSelectionRef(workflowVersionId, recipeId);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : "无法编码生成器身份");
    }
    return {
      line: row.line, shotId: shot?.id ?? "", shotName: shot?.name ?? "",
      shotOrdinal: shot ? shot.ordinal + 1 : (row.shotOrdinal ?? 0),
      inputKey: (inputKey ?? "first_frame") as BatchBindKey, assetId: row.assetId ?? "",
      workflowVersionId, recipeId, selectionRef, errors,
    };
  });
}

export function mergeShotBindings(existing: VideoInputAsset[], inputKey: BatchBindKey, assetId: string): { inputs: VideoInputAsset[]; error?: string } {
  const others = existing.filter((i) => i.inputKey !== inputKey);
  let nextKey: VideoInputAsset[];
  if (PLURAL.has(inputKey)) {
    const current = existing.filter((i) => i.inputKey === inputKey).sort((a, b) => a.ordinal - b.ordinal);
    nextKey = current.some((i) => i.assetId === assetId)
      ? current.map((i, ordinal) => ({ inputKey: i.inputKey, ordinal, assetId: i.assetId }))
      : [...current, { inputKey: inputKey as VideoInputKey, ordinal: current.length, assetId }];
    const max = SLOT_MAX[inputKey];
    if (max !== undefined && nextKey.length > max) return { inputs: existing, error: `${BATCH_BIND_KEY_LABELS[inputKey]}最多 ${max} 个。` };
  } else {
    nextKey = [{ inputKey: inputKey as VideoInputKey, ordinal: 0, assetId }];
  }
  const merged = [...others, ...nextKey];
  const images = merged.filter((i) => i.inputKey === "reference_images").length;
  const videos = merged.filter((i) => i.inputKey === "reference_videos").length;
  const audios = merged.filter((i) => i.inputKey === "reference_audios").length;
  if (images > 9 || videos > 3 || audios > 3 || images + videos + audios > 12 || (audios > 0 && images + videos === 0)) {
    return { inputs: existing, error: "参考图最多 9 个、参考视频最多 3 个、参考音频最多 3 个，合计不超过 12 个，且不能只有音频。" };
  }
  return { inputs: merged };
}

export interface ShotBindPlan {
  shotId: string; shotName: string; shotOrdinal: number; selectionRef: string;
  workflowVersionId: string; recipeId: string;
  assignments: Array<{ line: number; inputKey: BatchBindKey; assetId: string }>;
}
export function groupAssignmentsByShot(assignments: ResolvedBindAssignment[]): ShotBindPlan[] {
  const order: string[] = [];
  const map = new Map<string, ShotBindPlan>();
  for (const row of assignments) {
    if (row.errors.length || !row.shotId || !row.selectionRef) continue;
    let plan = map.get(row.shotId);
    if (!plan) {
      plan = { shotId: row.shotId, shotName: row.shotName, shotOrdinal: row.shotOrdinal, selectionRef: row.selectionRef, workflowVersionId: row.workflowVersionId, recipeId: row.recipeId, assignments: [] };
      map.set(row.shotId, plan); order.push(row.shotId);
    }
    plan.assignments.push({ line: row.line, inputKey: row.inputKey, assetId: row.assetId });
  }
  return order.map((id) => map.get(id)!);
}

export function shotsReadyForBatchBind(shots: ShotView[], catalog: RecipeViewModel[]) {
  return shots.slice().sort((a, b) => a.ordinal - b.ordinal || a.id.localeCompare(b.id)).flatMap((shot) => {
    const recipe = recipeForShot(shot, catalog);
    if (!recipe?.persistentInputs) return [];
    const slots = recipeMediaSlots(recipe);
    if (!slots.length) return [];
    const config = videoStageConfig(shot)!;
    const selectionRef = recipe.selectionRef?.trim() || encodeExactSelectionRef(config.workflowVersionId, config.recipeId);
    return [{ shot, recipe, slots, selectionRef }];
  });
}
