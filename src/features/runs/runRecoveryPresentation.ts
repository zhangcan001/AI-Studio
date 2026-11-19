import type { ProductRun } from "../../product/types";

// Presentation only: backend capabilities, never error text, determine recovery.
export function runRecoveryPresentation(run: ProductRun) {
  const showRecovery = ["FAILED", "PARTIAL", "PAUSED", "CANCELLED"].includes(run.status);
  const retryCount = run.availableActions.includes("RETRY") ? run.recoverability.retryItemIds.length : 0;
  const needsInputEdit = run.recoverability.reviewRequired > 0 || run.availableActions.includes("EDIT_INPUT");
  const disconnectPause = run.status === "PAUSED" && run.availableActions.includes("DISCONNECT_PAUSE");
  const unknownFailure = showRecovery && run.status !== "PAUSED" && run.progress.failed > 0 && !retryCount && !needsInputEdit;
  return {
    showRecovery,
    disconnectPause,
    heading: disconnectPause ? "连接中断后的暂停" : run.status === "PAUSED" ? "暂停与继续" : run.status === "CANCELLED" ? "取消与保留" : "失败与恢复",
    retry: showRecovery && retryCount > 0 ? {
      count: retryCount,
      label: "重试原运行的可恢复失败项",
      explanation: run.ref.source === "production-run"
        ? "旧生产运行的重试会按原快照立即启动生成，不会采用当前 Create 中的修改；历史尝试与已完成结果会保留。"
        : "重试使用这些失败项原来的运行快照，不会采用当前 Create 中尚未提交的修改；历史尝试仍会保留。重试只会把失败项重新放入待启动队列，仍需再点「启动运行」后才会开始生成。",
      confirm: run.ref.source === "production-run"
        ? "将按原快照重试失败项并立即启动生成。历史尝试与已完成结果会保留。确定重试并启动？"
        : "重试只会按原快照把失败项重新放入待启动队列，不会自动开始生成。仍需再点「启动运行」。确定继续？",
    } : undefined,
    needsInputEdit: showRecovery && needsInputEdit,
    inputEditExplanation: "有失败项需要修改输入。这类问题不能通过原快照重试解决，请在下方选择具体输入进行编辑。",
    summary: disconnectPause ? "与 ComfyUI 的连接已断开，批次已暂停。失败的镜头不会被「继续运行」覆盖，请先重试失败项，再启动运行。"
      : run.status === "PAUSED" ? "运行已暂停，不代表已完成或失败；暂停不抢占当前推理，只有显式点击继续运行才会调度后续排队项。"
      : run.status === "CANCELLED" ? "运行已取消；已完成的结果与历史仍保留。"
      : retryCount > 0 && needsInputEdit ? "部分失败项可以按原快照重试；另有部分失败项需要修改输入。"
      : retryCount > 0 ? "可安全恢复的失败项可以使用原运行快照重试。"
      : needsInputEdit ? "请检查需要修改的历史输入，再确认是否创建新运行。"
      : run.progress.failed > 0 ? "当前没有可自动重试的失败项。原因暂未分类，可查看输入详情与高级技术诊断。"
      : "当前没有可自动重试的失败项；已有状态和历史仍会保留。",
    unknownFailure,
    resultsPreserved: run.status === "PARTIAL" || run.progress.succeeded > 0 || run.resultsSummary.length > 0,
  };
}
