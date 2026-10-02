import type { AppRoute } from "../../app/routes/types";
import type { ProductRun, RunInput, RunListFilter, RunRef } from "../../product/types";
export type RunsRoute = Extract<AppRoute, { kind: "runs" }>;
export const runKey = (ref: RunRef) => `${ref.source}:${ref.id}`;
export const labels: Record<ProductRun["status"], string> = { QUEUED: "排队中", RUNNING: "运行中", PAUSED: "已暂停", FAILED: "失败", PARTIAL: "部分完成", SUCCEEDED: "已完成", CANCELLED: "已取消" };
export const filters: { key: RunListFilter; label: string }[] = [{ key: "all", label: "全部" }, { key: "active", label: "进行中" }, { key: "failed", label: "失败" }, { key: "completed", label: "已完成" }];
export function filterFor(route: RunsRoute): RunListFilter { return filters.find(filter => filter.key === route.filter)?.key ?? "all"; }
export function normalRuns(route: AppRoute, shellMode: string) { return shellMode === "v3" && route.kind === "runs" && !["production", "review", "tasks"].includes(route.filter ?? ""); }
export const friendlyGenerator = (name: string) => /(?:^wfl_|^wfv_|\.json$|[\\/]|[0-9a-f]{32})/i.test(name) ? /h3|minimax/i.test(name) ? "H3 视频生成器" : /krea/i.test(name) ? "Krea 图片生成器" : "生成器" : name;
export function inputText(input: RunInput) { return Object.entries(input.values).map(([key, value]) => ({ key, label: ({ prompt: "提示词", negative_prompt: "负向提示词", duration_seconds: "时长（秒）", width: "宽度", height: "高度", steps: "步数", cfg: "引导强度", seed: "种子" } as Record<string, string>)[key] ?? "参数", value: "assetId" in value ? "1 个参考素材" : "assetIds" in value ? `${value.assetIds.length} 个参考素材` : "value" in value ? String(value.value) : "随机" })); }
