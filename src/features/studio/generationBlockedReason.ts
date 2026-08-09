interface GenerationBlockedReasonInput {
  productionBusy: boolean;
  comfyConnected: boolean;
  taskEventsReady: boolean;
  taskEventError?: string;
  missingAsset: boolean;
  validationError: boolean;
  unsupportedField: boolean;
}

export function generationBlockedReason({
  productionBusy,
  comfyConnected,
  taskEventsReady,
  taskEventError,
  missingAsset,
  validationError,
  unsupportedField,
}: GenerationBlockedReasonInput): string | undefined {
  if (productionBusy) return "生产队列正在运行，请先暂停或等待完成。";
  if (!comfyConnected) return "ComfyUI 当前离线，请连接后再开始生成。";
  if (!taskEventsReady) return taskEventError ?? "任务事件通道正在准备，请稍后再试。";
  if (missingAsset) return "请先选择所需素材。";
  if (validationError) return "请检查标红字段后再开始生成。";
  if (unsupportedField) return "当前工作流包含暂不支持的输入类型。";
  return undefined;
}
