import type { ProductRun } from "../../product/types";

// Presentation only: backend capabilities, never error text, determine recovery.
export function runRecoveryPresentation(run: ProductRun) {
  const showRecovery = ["FAILED", "PARTIAL", "PAUSED", "CANCELLED"].includes(run.status);
  const retryCount = run.availableActions.includes("RETRY") ? run.recoverability.retryItemIds.length : 0;
  const needsInputEdit = run.recoverability.reviewRequired > 0 || run.availableActions.includes("EDIT_INPUT");
  const unknownFailure = showRecovery && run.status !== "PAUSED" && run.progress.failed > 0 && !retryCount && !needsInputEdit;
  return {
    showRecovery,
    retry: showRecovery && retryCount > 0 ? {
      count: retryCount,
      label: "重试原运行的可恢复失败项",
      explanation: "重试使用这些失败项原来的运行快照，不会采用当前 Create 中尚未提交的修改；历史尝试仍会保留。",
    } : undefined,
    needsInputEdit: showRecovery && needsInputEdit,
    inputEditExplanation: "有失败项需要修改输入。这类问题不能通过原快照重试解决，请在下方选择具体输入进行编辑。",
    summary: run.status === "PAUSED" ? "运行已暂停，不代表已完成或失败；只有点击继续运行才会恢复执行。"
      : retryCount > 0 && needsInputEdit ? "部分失败项可以按原快照重试；另有部分失败项需要修改输入。"
      : retryCount > 0 ? "可安全恢复的失败项可以使用原运行快照重试。"
      : needsInputEdit ? "请检查需要修改的历史输入，再确认是否创建新运行。"
      : run.progress.failed > 0 ? "当前没有可自动重试的失败项。原因暂未分类，可查看输入详情与高级技术诊断。"
      : "当前没有可自动重试的失败项；已有状态和历史仍会保留。",
    unknownFailure,
    resultsPreserved: run.status === "PARTIAL" || run.progress.succeeded > 0 || run.resultsSummary.length > 0,
  };
}
