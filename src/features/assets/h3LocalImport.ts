import type { H3LocalImportInspection, H3LocalImportMode, H3LocalPairStatus } from "../../types/h3LocalImport";

export function localImportModeLabel(mode: H3LocalImportMode): string {
  return mode === "PAIRING" ? "自动同名配对" : "JSON 批量清单";
}

export function localImportStatusLabel(status: H3LocalPairStatus): string {
  switch (status) {
    case "READY": return "可生成";
    case "MISSING_PROMPT": return "缺少 Prompt";
    case "MISSING_IMAGE": return "缺少图片";
    case "AMBIGUOUS_PROMPT": return "Prompt 不唯一";
    case "AMBIGUOUS_IMAGE": return "图片不唯一";
    case "INVALID_PROMPT_ENCODING": return "Prompt 编码无效";
    case "EMPTY_PROMPT": return "Prompt 为空";
    case "PROMPT_TOO_LARGE": return "Prompt 超过 64 KiB";
    case "INVALID_IMAGE": return "图片无效";
    case "IMAGE_TOO_LARGE": return "图片过大";
    case "INVALID_PATH": return "路径不安全";
    case "DUPLICATE_IMAGE_ENTRY": return "清单重复图片";
    case "UNKNOWN_IMAGE": return "图片不存在";
  }
}

export function localImportCanCommit(
  inspection: H3LocalImportInspection | undefined,
  runtimeReady: boolean,
  admissionBusy: boolean,
): boolean {
  return Boolean(
    inspection
      && inspection.errorCount === 0
      && inspection.readyCount > 0
      && inspection.readyCount <= 100
      && runtimeReady
      && !admissionBusy,
  );
}

export function formatPromptBytes(bytes: number | undefined): string {
  if (bytes === undefined) return "—";
  return `${bytes.toLocaleString()} B`;
}
