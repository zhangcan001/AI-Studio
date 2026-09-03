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
  COMFY_ENDPOINT_INVALID: "ComfyUI 地址无效，请使用不含账号、参数和片段的 http 或 https 地址。",
  COMFY_ENDPOINT_TEST_FAILED: "无法连接到该 ComfyUI 地址。",
  COMFY_ENDPOINT_CHANGE_BUSY: "当前仍有生成任务或生产队列正在运行，完成后才能切换 ComfyUI。",
  SETTINGS_SAVE_FAILED: "设置保存失败，请检查本地文件权限后重试。",
  BACKUP_INVALID: "项目备份无效或不受支持，请选择 AI Studio 导出的备份文件。",
  BACKUP_INSPECTION_EXPIRED: "备份预览已过期，请重新选择备份文件。",
  BACKUP_ASSET_HASH_MISMATCH: "项目备份中的素材校验失败，恢复已取消。",
  BACKUP_SNAPSHOT_ASSET_REMAP_FAILED: "项目备份中的历史素材引用无法安全恢复，恢复已取消。",
  ASSET_DELETION_BLOCKED: "所选素材仍被活动任务或生产队列使用，请完成或取消后再删除。",
  FILESYSTEM_BOUNDARY_ERROR: "素材文件路径不在当前项目目录内，删除已阻止。",
  COMFY_MEMORY_BUSY: "当前仍有任务或 ComfyUI 队列活动，完成或取消后再释放模型内存。",
  COMFY_MEMORY_RELEASE_FAILED: "ComfyUI 释放显存/内存失败，请检查连接后重试。",
  PRODUCTION_RUNTIME_NOT_READY: "无法启动生产队列：运行时准入检查未通过。",
  PRODUCTION_START_ADMISSION_BLOCKED: "无法启动生产队列：运行时准入检查未通过。",
  RUNTIME_ADMISSION_RECIPE_NOT_FOUND: "无法启动生产队列：请求的工作流配方不存在。",
  RUNTIME_ADMISSION_WORKFLOW_NOT_FOUND: "无法启动生产队列：请求的工作流版本不存在。",
  RUNTIME_ADMISSION_WORKFLOW_DISABLED: "无法启动生产队列：请求的工作流版本已停用。",
  RUNTIME_ADMISSION_WORKFLOW_ARCHIVED: "无法启动生产队列：请求的工作流版本已归档。",
  RUNTIME_ADMISSION_PACKAGE_INVALID: "无法启动生产队列：工作流运行包校验未通过。",
  RUNTIME_ADMISSION_MISSING_NODES: "无法启动生产队列：ComfyUI 缺少工作流节点。",
  RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE: "无法启动生产队列：工作流输入与当前 ComfyUI 能力不兼容。",
  RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED: "无法启动生产队列：尚未完成当前工作流的运行环境检查。",
  RUNTIME_ADMISSION_COMFY_UNAVAILABLE: "无法启动生产队列：ComfyUI 当前不可用。",
  RUNTIME_ADMISSION_COMFY_INCOMPATIBLE: "无法启动生产队列：ComfyUI 返回了不兼容的运行时响应。",
  RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED: "无法启动生产队列：ComfyUI 能力刷新失败。",
  RUNTIME_ADMISSION_WORKSPACE_DIAGNOSTICS_FAILED: "无法启动生产队列：工作流运行包诊断失败。",
  RUNTIME_ADMISSION_CAPABILITY_OFFLINE: "无法启动生产队列：工作流能力检查发现 ComfyUI 离线。",
  RUNTIME_ADMISSION_CAPABILITY_UNKNOWN: "无法启动生产队列：工作流能力状态未知。",
  RUNTIME_ADMISSION_DIAGNOSTICS: "无法启动生产队列：工作流运行包存在诊断问题。",
  RUNTIME_ADMISSION_READINESS_BLOCKED: "无法启动生产队列：工作流尚未达到生产就绪状态。",
  QUEUE_RUNTIME_NOT_READY: "无法启动生产队列：引用的运行时尚未就绪。",
  QUEUE_RUNTIME_CAPABILITY_INVALID: "无法启动生产队列：ComfyUI 能力检查未通过。",
  QUEUE_RUNTIME_UNAVAILABLE: "无法启动生产队列：引用的运行时不可用。",
  QUEUE_RUNTIME_DISABLED: "无法启动生产队列：引用的运行时已停用。",
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
  WORKFLOW_NOT_API_FORMAT: "该文件不是 ComfyUI API 格式工作流，请重新导出 API 格式工作流。",
  WORKFLOW_FILE_TOO_LARGE: "工作流文件过大，无法导入。",
  WORKFLOW_PACKAGE_INVALID: "工作流运行包无效，请重新检查工作流。",
  WORKFLOW_VERSION_CONFLICT: "工作流版本已发生变化，请刷新后重试。",
  RECIPE_VERSION_CONFLICT: "配方版本已发生变化，请刷新后重试。",
  WORKFLOW_VALIDATION_FAILED: "工作流校验未通过，请检查输入和输出映射。",
  REFERENCE_MAPPING_INCOMPLETE: "参考图绑定不完整，请补齐结构化素材绑定后再生成。",
  WORKFLOW_ONBOARDING_ERROR: "工作流导入失败，请检查文件和映射配置。",
  MISSING_NODE: "当前 ComfyUI 缺少该工作流需要的节点。",
  INPUT_OPTION_UNAVAILABLE: "当前 ComfyUI 中缺少工作流所需的模型或选项。",
  INPUT_REQUIRED: "请先填写必填输入项。",
  INPUT_TYPE_MISMATCH: "输入值类型不符合要求。",
  INPUT_OUT_OF_RANGE: "输入值超出允许范围。",
  INPUT_STEP_MISMATCH: "输入值不符合当前配方的步进要求。",
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
  PROMPT_TEMPLATE_SYNTAX_ERROR: "模板语法有误，请检查 {{variable.path}} 是否完整。",
  PROMPT_TEMPLATE_UNKNOWN_VARIABLE: "模板包含尚未支持的变量，请修改后重试。",
  PROMPT_TEMPLATE_CONTEXT_MISSING: "当前镜头缺少模板所需的项目结构上下文。",
  PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING: "请填写模板要求的自定义变量。",
  PROMPT_TEMPLATE_CUSTOM_VALUES_INVALID: "模板自定义变量格式无效，请检查名称、长度和总大小。",
  PROMPT_TEMPLATE_APPLY_VALIDATION_FAILED: "模板批量校验未通过，请检查镜头上下文与自定义变量。",
  PROMPT_TEMPLATE_ANCHOR_PROJECT_MISMATCH: "所选参考锚点不属于当前项目。",
  PROMPT_TEMPLATE_ANCHOR_LIMIT: "模板上下文最多选择 20 个参考锚点。",
  PROMPT_TEMPLATE_SHOT_LIMIT: "模板批量应用一次最多处理 500 个镜头。",
  PROMPT_TEMPLATE_RESULT_TOO_LARGE: "渲染后的提示词过长，请缩短模板或上下文。",
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
    if (typeof code === "string" && code && code !== "INVALID_INPUT") return code;
  }
  const embeddedRuntimeCode = raw.match(/\bRUNTIME_ADMISSION_[A-Z0-9_]+\b/)?.[0];
  if (embeddedRuntimeCode && RUNTIME_ADMISSION_CODES.has(embeddedRuntimeCode)) return embeddedRuntimeCode;
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as { code?: unknown }).code;
    if (typeof code === "string" && code) return code;
  }
  return raw.match(/^[A-Z][A-Z0-9_]{2,}/)?.[0];
}

const RUNTIME_ADMISSION_CODES = new Set([
  "PRODUCTION_RUNTIME_NOT_READY",
  "PRODUCTION_START_ADMISSION_BLOCKED",
  "RUNTIME_ADMISSION_RECIPE_NOT_FOUND",
  "RUNTIME_ADMISSION_WORKFLOW_NOT_FOUND",
  "RUNTIME_ADMISSION_WORKFLOW_DISABLED",
  "RUNTIME_ADMISSION_WORKFLOW_ARCHIVED",
  "RUNTIME_ADMISSION_PACKAGE_INVALID",
  "RUNTIME_ADMISSION_MISSING_NODES",
  "RUNTIME_ADMISSION_CAPABILITY_INCOMPATIBLE",
  "RUNTIME_ADMISSION_CAPABILITY_NOT_CHECKED",
  "RUNTIME_ADMISSION_COMFY_UNAVAILABLE",
  "RUNTIME_ADMISSION_COMFY_INCOMPATIBLE",
  "RUNTIME_ADMISSION_CAPABILITY_REFRESH_FAILED",
  "RUNTIME_ADMISSION_WORKSPACE_DIAGNOSTICS_FAILED",
  "RUNTIME_ADMISSION_CAPABILITY_OFFLINE",
  "RUNTIME_ADMISSION_CAPABILITY_UNKNOWN",
  "RUNTIME_ADMISSION_DIAGNOSTICS",
  "RUNTIME_ADMISSION_READINESS_BLOCKED",
  "QUEUE_RUNTIME_NOT_READY",
  "QUEUE_RUNTIME_CAPABILITY_INVALID",
  "QUEUE_RUNTIME_UNAVAILABLE",
  "QUEUE_RUNTIME_DISABLED",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function stringList(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(nonEmptyString).filter((item): item is string => Boolean(item));
  const single = nonEmptyString(value);
  return single ? [single] : [];
}

function runtimeAdmissionRecords(error: unknown): Record<string, unknown>[] {
  if (!isRecord(error) || !("details" in error)) return [];
  const details = error.details;
  if (Array.isArray(details)) return details.filter(isRecord);
  if (!isRecord(details)) return [];
  for (const key of ["failures", "issues", "blockers"]) {
    const records = details[key];
    if (Array.isArray(records)) return records.filter(isRecord);
  }
  return [details];
}

function runtimeAdmissionDetails(error: unknown): string | undefined {
  const descriptions = runtimeAdmissionRecords(error).map((record) => {
    const workflowVersionId = nonEmptyString(record.workflowVersionId ?? record.workflow_version_id);
    const recipeId = nonEmptyString(record.recipeId ?? record.recipe_id);
    const missingNodes = stringList(record.missingNodes ?? record.missingNodeIds ?? record.missing_node_ids);
    const message = nonEmptyString(record.message ?? record.reason);
    return [
      workflowVersionId ? `工作流版本 ${workflowVersionId}` : undefined,
      recipeId ? `配方 ${recipeId}` : undefined,
      missingNodes.length ? `缺少节点：${missingNodes.join("、")}` : undefined,
      message,
    ].filter(Boolean).join("；");
  }).filter(Boolean);
  return descriptions.length ? descriptions.join("；") : undefined;
}

function runtimeAdmissionTextDetails(technicalMessage: string): string | undefined {
  const workflowVersionId = technicalMessage.match(/workflow_version_id=([^,\s]+)/)?.[1];
  const recipeId = technicalMessage.match(/recipe_id=([^,\s]+)/)?.[1];
  const missingNodes = technicalMessage.match(/missing_nodes=([^,\s]+)/)?.[1]?.split(",").filter(Boolean) ?? [];
  const reason = technicalMessage.match(/reason=(.*?)(?:,\s+missing_nodes=|$)/)?.[1];
  const details = [
    workflowVersionId ? `工作流版本 ${workflowVersionId}` : undefined,
    recipeId ? `配方 ${recipeId}` : undefined,
    missingNodes.length ? `缺少节点：${missingNodes.join("、")}` : undefined,
    reason,
  ].filter(Boolean);
  return details.length ? details.join("；") : undefined;
}

function runtimeAdmissionMessage(error: unknown, code: string | undefined, technicalMessage: string): string | undefined {
  if (!code || !RUNTIME_ADMISSION_CODES.has(code)) return undefined;
  const base = ERROR_MESSAGES[code] ?? ERROR_MESSAGES.PRODUCTION_RUNTIME_NOT_READY;
  const details = runtimeAdmissionDetails(error) ?? runtimeAdmissionTextDetails(technicalMessage);
  if (details) return `${base}（${details}）`;
  const suffix = technicalMessage.replace(/^[A-Z][A-Z0-9_]{2,}\s*[:：]\s*/, "").trim();
  if (suffix && !/^runtime admission blocked$/i.test(suffix) && suffix !== base) {
    return `${base}（${suffix}）`;
  }
  return base;
}

export function formatUiError(error: unknown): UiError {
  const technicalMessage = rawErrorMessage(error);
  const code = errorCode(error, technicalMessage);
  const message = runtimeAdmissionMessage(error, code, technicalMessage)
    ?? ((code && ERROR_MESSAGES[code]) ?? "操作失败，请查看技术详情。");
  return { message, code, technicalMessage };
}

export function toUserMessage(error: unknown): string {
  return formatUiError(error).message;
}

export function errorMessageForCode(code: string, fallback?: string): string {
  return ERROR_MESSAGES[code] ?? fallback ?? "操作失败，请查看技术详情。";
}
