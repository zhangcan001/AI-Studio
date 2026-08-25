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
      return "基准实验已开始，候选将按普通生产队列串行执行。";
    case "QUEUED":
      return autoStart
        ? "基准实验已加入队列，但当前生产准入繁忙，等待队列启动。"
        : "基准实验已加入生产队列，尚未启动。";
    case "COMPLETED":
      return "基准实验已完成。";
    case "PARTIAL":
      return "基准实验已部分完成。";
    case "CANCELLED":
      return "基准实验已取消。";
    case "FAILED_TO_QUEUE":
      return "基准实验未能加入生产队列，请检查生产准入。";
    default:
      return "基准实验当前状态：未知状态";
  }
}

export function canRunBenchmarkDraft(status: string, productionBatchId?: string): boolean {
  return status === "DRAFT" && !productionBatchId;
}
