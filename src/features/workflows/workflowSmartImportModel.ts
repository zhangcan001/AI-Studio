import { formatUiError } from "../../i18n/errorMessages";
import type { WorkflowImportErrorView } from "./WorkflowImportIssues";

export function workflowImportErrorView(error: unknown): WorkflowImportErrorView {
  const formatted = formatUiError(error);
  const haystack = `${formatted.code ?? ""} ${formatted.technicalMessage}`.toUpperCase();
  if (/INVALID[_\s-]*JSON|JSON[_\s-]*(PARSE|INVALID)|MALFORMED[_\s-]*JSON/.test(haystack)) {
    return { kind: "INVALID_JSON", message: "无法读取这个文件，它不是有效的 JSON。" };
  }
  if (/UI[_\s-]*(FORMAT|WORKFLOW)|WORKFLOW[_\s-]*UI|UNSUPPORTED[_\s-]*UI/.test(haystack)) {
    return { kind: "UI_FORMAT", message: "检测到 ComfyUI 普通工作流 JSON，但这个格式不能安全地直接添加。" };
  }
  if (/\bUNKNOWN\b|UNRECOGNIZED|WORKFLOW_NOT_API_FORMAT/.test(haystack)) {
    return { kind: "UNKNOWN_FORMAT", message: "这个 JSON 不是可识别的 ComfyUI 工作流。" };
  }
  return {
    kind: "IMPORT_FAILED",
    message: formatted.message === "操作失败，请查看技术详情。"
      ? "工作流导入未完成，请查看详细原因后重试。"
      : formatted.message,
    code: formatted.code,
    technicalMessage: formatted.technicalMessage,
  };
}

export function nextWorkflowVersion(value: string): string {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(value.trim());
  if (!match) return "1.0.1";
  return `${match[1]}.${match[2]}.${Number(match[3]) + 1}`;
}
