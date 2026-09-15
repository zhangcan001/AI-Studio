import type { ProductionBatchDetail } from "../../types/productionQueue";

export function productionBatchOutcomeLabel(
  detail: Pick<ProductionBatchDetail, "status" | "total" | "succeeded" | "failed" | "cancelled" | "skipped">,
): string {
  if (detail.status !== "COMPLETED") return "";
  if (
    detail.total > 0
    && detail.succeeded === detail.total
    && detail.failed === 0
    && detail.cancelled === 0
    && detail.skipped === 0
  ) {
    return `全部生成成功 · ${detail.succeeded} 项`;
  }

  const processed = detail.succeeded + detail.failed + detail.cancelled + detail.skipped;
  const counts = [`成功 ${detail.succeeded} 项`];
  if (detail.failed > 0) counts.push(`失败 ${detail.failed} 项`);
  if (detail.cancelled > 0) counts.push(`取消 ${detail.cancelled} 项`);
  if (detail.skipped > 0) counts.push(`跳过 ${detail.skipped} 项`);
  return `处理结束 · ${processed}/${detail.total} 已处理 · ${counts.join("，")}`;
}
