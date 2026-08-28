export type ProjectImportFormat = "json" | "tsv";
export type ImportIssueSeverity = "error" | "warning" | "info";

export interface ProjectImportPreviewIssue {
  code: string;
  message: string;
  rowNumber?: number;
  shotId?: string;
}

export interface ProjectImportPreviewRow {
  rowNumber: number;
  name: string;
  description: string;
  imagePrompt?: string;
  videoPrompt?: string;
  errors: ProjectImportPreviewIssue[];
  warnings: ProjectImportPreviewIssue[];
}

export interface ProjectImportPreview {
  total: number;
  valid: number;
  invalid: number;
  warnings: number;
  rows: ProjectImportPreviewRow[];
}

export interface ImportIssue {
  id: string;
  severity: ImportIssueSeverity;
  blocking: boolean;
  entityType?: string;
  entityId?: string;
  row?: number;
  index?: number;
  field?: string;
  code?: string;
  message: string;
}

export interface ImportReadiness {
  ready: boolean;
  totalRecords: number;
  validRecords: number;
  invalidRecords: number;
  errorCount: number;
  warningCount: number;
  blockingCount: number;
}

export interface ImportDryRunSummary {
  createCount?: number;
  updateCount?: number;
  unchangedCount?: number;
  skippedCount?: number;
}

export interface ImportDryRunResult {
  readiness: ImportReadiness;
  issues: ImportIssue[];
  summary: ImportDryRunSummary;
  rows?: ProjectImportPreviewRow[];
}

export function importFormatForFileName(fileName: string): ProjectImportFormat | undefined {
  const extension = fileName.trim().toLowerCase().split(".").pop();
  if (extension === "json") return "json";
  if (extension === "tsv" || extension === "txt") return "tsv";
  return undefined;
}

export function normalizeImportContent(content: string): string {
  return content.replace(/^\uFEFF/, "");
}

export function buildImportDryRunResult(preview: ProjectImportPreview): ImportDryRunResult {
  const issues = preview.rows.flatMap((row) => [
    ...row.errors.map((issue, issueIndex) => ({
      id: `row-${row.rowNumber}-error-${issueIndex}`,
      severity: "error" as const,
      blocking: true,
      entityType: "shot",
      entityId: issue.shotId,
      row: issue.rowNumber ?? row.rowNumber,
      code: issue.code,
      message: issue.message,
    })),
    ...row.warnings.map((issue, issueIndex) => ({
      id: `row-${row.rowNumber}-warning-${issueIndex}`,
      severity: "warning" as const,
      blocking: false,
      entityType: "shot",
      entityId: issue.shotId,
      row: issue.rowNumber ?? row.rowNumber,
      code: issue.code,
      message: issue.message,
    })),
  ] satisfies ImportIssue[]);
  const errorCount = issues.filter((issue) => issue.severity === "error").length;
  const warningCount = Math.max(
    preview.warnings,
    issues.filter((issue) => issue.severity === "warning").length,
  );
  const blockingCount = issues.filter((issue) => issue.blocking).length;
  const ready = preview.total > 0 && preview.invalid === 0 && blockingCount === 0;

  return {
    readiness: {
      ready,
      totalRecords: preview.total,
      validRecords: preview.valid,
      invalidRecords: preview.invalid,
      errorCount,
      warningCount,
      blockingCount,
    },
    issues,
    // The existing importer is CREATE ONLY. When validation blocks the
    // atomic commit, no record will be written, so do not invent a partial
    // create/update estimate.
    summary: ready ? { createCount: preview.valid } : {},
    rows: preview.rows,
  };
}

export function failedImportResult(issue: Omit<ImportIssue, "id">): ImportDryRunResult {
  return {
    readiness: {
      ready: false,
      totalRecords: 0,
      validRecords: 0,
      invalidRecords: 0,
      errorCount: issue.severity === "error" ? 1 : 0,
      warningCount: issue.severity === "warning" ? 1 : 0,
      blockingCount: issue.blocking ? 1 : 0,
    },
    issues: [{ ...issue, id: `file-${issue.code ?? "error"}` }],
    summary: {},
  };
}

export function importIssueFromError(error: unknown): ImportIssue {
  const rawMessage = error instanceof Error
    ? error.message
    : error && typeof error === "object" && "message" in error && typeof error.message === "string"
      ? error.message
      : typeof error === "string"
        ? error
        : "导入预检失败，请检查文件后重试。";
  const code = error && typeof error === "object" && "code" in error && typeof error.code === "string"
    ? error.code
    : rawMessage.match(/[A-Z][A-Z0-9_]{2,}/)?.[0] ?? "IMPORT_PREVIEW_FAILED";
  return {
    id: `file-${code}`,
    severity: "error",
    blocking: true,
    code,
    message: rawMessage,
  };
}
