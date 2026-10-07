import type { ProductErrorDetails } from "../../product/errors";

/** Create-only presentation; the canonical router and existing controller own effects. */
export type CreateReadinessAction =
  | { kind: "focus-field"; field: string; label: string }
  | { kind: "select-generator" | "open-runtime-settings" | "recheck" | "open-projects" | "focus-inputs"; label: string }
  | { kind: "none"; explanation: string };

export function resolveCreateReadinessAction(details: ProductErrorDetails): CreateReadinessAction {
  switch (details.action) {
    case "EDIT_INPUT": return details.field ? { kind: "focus-field", field: details.field, label: "修改此输入" } : { kind: "focus-inputs", label: "检查输入" };
    case "SELECT_GENERATOR": return { kind: "select-generator", label: "重新选择生成器" };
    case "OPEN_RUNTIME_SETTINGS": return { kind: "open-runtime-settings", label: "检查运行环境" };
    case "TRY_LATER": return { kind: "recheck", label: "重新检查" };
    case "OPEN_PROJECTS": return { kind: "open-projects", label: "返回项目列表" };
    default: return { kind: "none", explanation: "无法自动定位，请根据提示检查当前输入或运行环境。" };
  }
}

export function focusCreateInput(field?: string) {
  const target = field ? document.getElementById(`create-field-${field}`) ??
    (["width", "height"].includes(field) ? document.getElementById("create-field-resolution") : null) : null;
  // Advanced parameters may be inside closed details; make the existing control reachable.
  for (let parent = target?.parentElement; parent; parent = parent.parentElement) {
    if (parent instanceof HTMLDetailsElement) parent.open = true;
  }
  if (target && !(target instanceof HTMLInputElement && target.disabled)) {
    target.focus();
    if (document.activeElement === target) return;
  }
  document.getElementById("create-inputs")?.focus();
}
