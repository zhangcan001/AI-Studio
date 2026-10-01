import type { GeneratorBindingSummary, RunRef } from "./types";

const messages = {
  PROJECT_NOT_FOUND: "项目不存在，请返回项目列表。",
  PROJECT_SCOPE_VIOLATION: "无法访问其他项目的数据。",
  GENERATOR_UNAVAILABLE: "生成器当前不可用，请重新选择。",
  GENERATOR_BINDING_CONFLICT: "生成器设置已被更新，请查看最新设置后再保存。",
  RUN_NOT_FOUND: "运行不存在或不属于当前项目。",
  RUN_NOT_RETRYABLE: "当前运行不能自动恢复，请检查运行状态。",
  EDIT_INPUT_REQUIRED: "请先修正输入，再创建新的生成任务。",
  INVALID_INPUT: "输入无效，请检查后重试。",
  PROJECT_READ_FAILED: "暂时无法读取项目，请稍后重试。",
  INTERNAL_ERROR: "暂时无法完成操作，请稍后重试。",
} as const;
export type ProductErrorCode = keyof typeof messages;
export interface ProductErrorDetails {
  actionLocation?: { projectId: string; runRef: RunRef; shotId: string | null; stage: string | null };
  field?: string;
  action?: string;
  retryable: boolean;
  currentBinding?: GeneratorBindingSummary;
  technicalDetails?: unknown;
}
export class ProductError extends Error {
  constructor(readonly code: ProductErrorCode, message: string, readonly details: ProductErrorDetails) {
    super(message);
    this.name = "ProductError";
  }
}
function record(value: unknown): value is Record<string, unknown> { return typeof value === "object" && value !== null; }
function knownCode(value: unknown): value is ProductErrorCode { return typeof value === "string" && Object.prototype.hasOwnProperty.call(messages, value); }
function binding(value: unknown): GeneratorBindingSummary | undefined {
  if (!record(value) || typeof value.stage !== "string" || typeof value.mode !== "string" || typeof value.selectionRef !== "string" || typeof value.bindingInstanceId !== "string" || typeof value.revision !== "number") return undefined;
  return { stage: value.stage, mode: value.mode, selectionRef: value.selectionRef, bindingInstanceId: value.bindingInstanceId, revision: value.revision };
}
function actionLocation(value: unknown): ProductErrorDetails["actionLocation"] {
  if (!record(value) || typeof value.projectId !== "string" || !record(value.runRef) || typeof value.runRef.id !== "string") return undefined;
  const source = value.runRef.source;
  if (source !== "task" && source !== "queue-batch" && source !== "production-run") return undefined;
  return { projectId: value.projectId, runRef: { source, id: value.runRef.id }, shotId: typeof value.shotId === "string" ? value.shotId : null, stage: typeof value.stage === "string" ? value.stage : null };
}
export function normalizeProductError(error: unknown): ProductError {
  if (error instanceof ProductError) return error;
  if (!record(error) || !knownCode(error.code)) return new ProductError("INTERNAL_ERROR", messages.INTERNAL_ERROR, { retryable: false, technicalDetails: error });
  const details = record(error.details) ? error.details : {};
  return new ProductError(error.code, messages[error.code], {
    actionLocation: actionLocation(details.actionLocation),
    retryable: details.retryable === true,
    field: typeof details.field === "string" ? details.field : undefined,
    action: typeof details.action === "string" ? details.action : undefined,
    currentBinding: binding(details.currentBinding),
    technicalDetails: details.technicalDetails,
  });
}
