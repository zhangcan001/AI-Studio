import type { CreationAsset, CreationContext, CreationShot, ProductRun } from "../../product/types";

/** Read model for the storyboard workbench. Derived only from product facade state; never invents shots. */
export type StoryboardShotStatus = "COMPLETED" | "GENERATING" | "REVIEW" | "FAILED" | "TODO";
export interface StoryboardShot {
  id: string;
  name: string;
  ordinal: number;
  shot: CreationShot | null;
  candidates: CreationAsset[];
  run: ProductRun | null;
  status: StoryboardShotStatus;
  error?: string;
}
export const storyboardStatusLabels: Record<StoryboardShotStatus, string> = { COMPLETED: "已完成", GENERATING: "生成中", REVIEW: "待审核", FAILED: "失败", TODO: "待制作" };
const ACTIVE = new Set<ProductRun["status"]>(["QUEUED", "RUNNING", "PAUSED"]);
export function deriveStoryboardStatus(shot: CreationShot | null, candidates: CreationAsset[], run: ProductRun | null): StoryboardShotStatus {
  if (run && ACTIVE.has(run.status)) return "GENERATING";
  if (shot?.selectedResultId || candidates.some(asset => asset.selected)) return "COMPLETED";
  if (candidates.length) return "REVIEW";
  if (run?.status === "FAILED") return "FAILED";
  return "TODO";
}
export function runPercent(run: ProductRun | null): number | null {
  if (!run || !run.progress.total) return null;
  return Math.round(((run.progress.succeeded + run.progress.failed + run.progress.cancelled) / run.progress.total) * 100);
}
export function coverAsset(item: Pick<StoryboardShot, "candidates">): CreationAsset | undefined {
  return item.candidates.find(asset => asset.selected) ?? item.candidates.find(asset => asset.thumbnailBytes?.length) ?? item.candidates[0];
}
function intValue(shot: CreationShot | null, key: string): number | undefined {
  const value = shot?.values[key];
  return value?.type === "integer" ? value.value : value?.type === "number" ? value.value : undefined;
}
export function shotDuration(shot: CreationShot | null) { const seconds = intValue(shot, "duration_seconds"); return seconds ? `${seconds} 秒` : "未设置"; }
export function shotResolution(shot: CreationShot | null) { const w = intValue(shot, "width"), h = intValue(shot, "height"); return w && h ? `${w} × ${h}` : "未设置"; }
export function storyboardStats(items: readonly StoryboardShot[]) {
  return { total: items.length, completed: items.filter(i => i.status === "COMPLETED").length, generating: items.filter(i => i.status === "GENERATING").length, todo: items.filter(i => i.status === "TODO" || i.status === "FAILED").length, review: items.filter(i => i.status === "REVIEW").length };
}
export function videoCandidates(context: CreationContext) { return context.candidates.filter(asset => asset.mediaKind === "video"); }
