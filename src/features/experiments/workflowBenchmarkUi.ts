import type { WorkflowBenchmarkCandidatePreview } from "../../types/benchmark";

export function previewForCandidatePosition(
  previews: WorkflowBenchmarkCandidatePreview[] | undefined,
  position: number,
): WorkflowBenchmarkCandidatePreview | undefined {
  return previews?.find((preview) => preview.position === position);
}

export function benchmarkAdmissionNotice(status: string, autoStart: boolean): string {
  switch (status) {
    case "RUNNING":
      return "Benchmark 已开始，候选将按普通生产队列串行执行。";
    case "QUEUED":
      return autoStart
        ? "Benchmark 已加入队列，但当前 Production Admission 繁忙，等待队列启动。"
        : "Benchmark 已加入生产队列，尚未启动。";
    case "COMPLETED":
      return "Benchmark 已完成。";
    case "PARTIAL":
      return "Benchmark 已部分完成。";
    case "CANCELLED":
      return "Benchmark 已取消。";
    case "FAILED_TO_QUEUE":
      return "Benchmark 未能加入生产队列，请检查 Production Admission。";
    default:
      return `Benchmark 当前状态：${status}`;
  }
}

export function canRunBenchmarkDraft(status: string, productionBatchId?: string): boolean {
  return status === "DRAFT" && !productionBatchId;
}
