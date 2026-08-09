export interface UiError {
  message: string;
  code?: string;
  technicalMessage?: string;
}

const ERROR_MESSAGES: Record<string, string> = {
  INITIALIZATION_ERROR: "应用初始化失败，请查看技术详情。",
  DATABASE_ERROR: "本地数据访问失败，请查看技术详情。",
  FILESYSTEM_ERROR: "本地文件访问失败，请检查文件权限后重试。",
  INTERNAL_ERROR: "应用内部操作失败，请查看技术详情。",
  COMFY_OFFLINE: "ComfyUI 当前离线，请先启动或重新连接 ComfyUI。",
  COMFY_TIMEOUT: "ComfyUI 响应超时，请检查运行状态后重试。",
  COMFY_PROTOCOL_ERROR: "ComfyUI 返回了不兼容的响应，请检查版本和 API 工作流格式。",
  COMFY_STREAM_DISCONNECTED: "与 ComfyUI 的执行连接已断开，请重新同步任务状态。",
  COMFY_INPUT_UPLOAD_FAILED: "素材上传到 ComfyUI 失败。",
  COMFY_IMAGE_UPLOAD_FAILED: "图片上传到 ComfyUI 失败。",
  COMFY_INPUT_UPLOAD_TOO_LARGE: "素材文件过大，无法上传到 ComfyUI。",
  PRODUCTION_QUEUE_BUSY: "当前有生产队列正在运行，请等待完成或暂停后再提交任务。",
  EXECUTION_ERROR: "生成执行失败，请查看任务详情中的技术信息。",
  EXECUTION_INTERRUPTED: "生成任务已被中断。",
  TASK_DOMAIN_ERROR: "任务状态操作失败，请查看技术详情。",
  TASK_NOT_CANCELLABLE: "当前任务状态不支持取消。",
  GENERATION_DEFINITION_NOT_FOUND: "当前工作流或配方不可用，请刷新工作流后重试。",
  INPUT_ASSET_NOT_FOUND: "找不到所选素材，请重新选择。",
  INPUT_ASSET_PROJECT_MISMATCH: "所选素材不属于当前项目，请重新选择。",
  INPUT_ASSET_TYPE_INVALID: "所选素材类型不符合当前输入要求。",
  INPUT_ASSET_READ_FAILED: "读取所选素材失败，请重新选择。",
  INPUT_ASSET_MIME_INVALID: "所选素材格式不受支持，请重新选择。",
  INPUT_ASSET_REPOSITORY_ERROR: "读取素材库失败，请稍后重试。",
  WORKFLOW_NOT_API_FORMAT: "该文件不是 ComfyUI API 格式工作流，请重新导出 API Format 工作流。",
  WORKFLOW_FILE_TOO_LARGE: "工作流文件过大，无法导入。",
  WORKFLOW_PACKAGE_INVALID: "工作流运行包无效，请重新检查工作流。",
  WORKFLOW_VERSION_CONFLICT: "工作流版本已发生变化，请刷新后重试。",
  RECIPE_VERSION_CONFLICT: "配方版本已发生变化，请刷新后重试。",
  WORKFLOW_VALIDATION_FAILED: "工作流校验未通过，请检查输入和输出映射。",
  WORKFLOW_ONBOARDING_ERROR: "工作流导入失败，请检查文件和映射配置。",
  MISSING_NODE: "当前 ComfyUI 缺少该工作流需要的节点。",
  INPUT_OPTION_UNAVAILABLE: "当前 ComfyUI 中缺少工作流所需的模型或选项。",
  INPUT_REQUIRED: "请先填写必填输入项。",
  INPUT_TYPE_MISMATCH: "输入值类型不符合要求。",
  INPUT_OUT_OF_RANGE: "输入值超出允许范围。",
  INPUT_COUNT_OUT_OF_RANGE: "输入素材数量不符合要求。",
  WORKFLOW_INVALID: "工作流内容无效，请重新导入。",
  QUEUE_DISPATCH_UNCERTAIN: "上次退出时任务提交结果无法确认，为避免重复生成，队列已暂停。",
  PRODUCTION_ADMISSION_RECOVERY_CONFLICT: "检测到多个历史生产任务仍处于活动状态，已暂停新的任务提交。",
  PRESET_NAME_REQUIRED: "请输入预设名称后再保存。",
  PROJECT_CONTEXT_CHANGED: "当前任务属于其他项目，请先切换到对应项目。",
  WORKFLOW_RUNTIME_NOT_FOUND: "找不到对应的工作流运行包，请刷新工作流列表。",
  WORKFLOW_VERSION_NOT_FOUND: "找不到对应的工作流版本，请刷新工作流列表。",
  PROJECT_NOT_FOUND: "找不到该项目，请刷新项目列表。",
  TASK_NOT_FOUND: "找不到该任务，请刷新任务历史。",
  ASSET_NOT_FOUND: "找不到该资产，请刷新资产库。",
  ASSET_READ_FAILED: "读取资产失败，请稍后重试。",
  ASSET_NOT_IMAGE: "所选资产不是图片。",
  REUSABLE_DRAFT_UNAVAILABLE: "保存的输入不可用于再次生成。",
  INVALID_INPUT: "输入内容无效，请检查后重试。",
  INVALID_PROJECT_ID: "项目标识无效，请重新选择项目。",
  INVALID_TASK_ID: "任务标识无效，请刷新任务历史。",
  INVALID_ASSET_ID: "资产标识无效，请刷新资产库。",
  TASK_STREAM_DISCONNECTED: "任务事件通道已断开，请刷新任务状态。",
  TASK_CANCEL_NOT_EFFECTIVE: "任务取消请求暂未生效，请刷新任务状态。",
  SNAPSHOT_INVALID: "任务恢复快照无效，请查看技术详情。",
  SNAPSHOT_PERSISTENCE_ERROR: "任务恢复状态保存失败，请查看技术详情。",
  TASK_RECOVERY_DEFERRED: "任务恢复已延后，请稍后刷新任务状态。",
  TASK_RECOVERY_UNRESOLVED: "任务恢复状态暂时无法确认，请查看任务详情。",
};

function rawErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function errorCode(error: unknown, raw: string): string | undefined {
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as { code?: unknown }).code;
    if (typeof code === "string" && code) return code;
  }
  return raw.match(/^[A-Z][A-Z0-9_]{2,}/)?.[0];
}

export function formatUiError(error: unknown): UiError {
  const technicalMessage = rawErrorMessage(error);
  const code = errorCode(error, technicalMessage);
  const message = (code && ERROR_MESSAGES[code]) ?? "操作失败，请查看技术详情。";
  return { message, code, technicalMessage };
}

export function toUserMessage(error: unknown): string {
  return formatUiError(error).message;
}

export function errorMessageForCode(code: string, fallback?: string): string {
  return ERROR_MESSAGES[code] ?? fallback ?? "操作失败，请查看技术详情。";
}
