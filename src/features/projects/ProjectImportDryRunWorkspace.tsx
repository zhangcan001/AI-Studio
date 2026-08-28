import { useRef, useState } from "react";
import {
  commitShotBulkImport,
  previewShotBulkImport,
} from "../../services/tauriClient";
import type {
  ImportDryRunResult,
  ImportIssue,
  ProjectImportFormat,
} from "../../types/projectImport";
import {
  buildImportDryRunResult,
  failedImportResult,
  importFormatForFileName,
  importIssueFromError,
  normalizeImportContent,
} from "../../types/projectImport";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import "./ProjectImportDryRunWorkspace.css";

type ImportStage = "idle" | "parsing" | "parsed" | "validating" | "validated" | "ready" | "executing" | "completed" | "failed";

interface Props {
  projectId: string;
  onClose: () => void;
  onImported?: () => void | Promise<void>;
}

export function ProjectImportDryRunWorkspace({ projectId, onClose, onImported }: Props) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const readRequestRef = useRef(0);
  const [stage, setStage] = useState<ImportStage>("idle");
  const [file, setFile] = useState<File>();
  const [format, setFormat] = useState<ProjectImportFormat>();
  const [content, setContent] = useState("");
  const [result, setResult] = useState<ImportDryRunResult>();
  const [executionResult, setExecutionResult] = useState<Awaited<ReturnType<typeof commitShotBulkImport>>>();
  const [error, setError] = useState<unknown>();

  const busy = stage === "parsing" || stage === "validating" || stage === "executing";
  const hasFormalImportApi = typeof commitShotBulkImport === "function";
  const canExecuteImport = Boolean(
    hasFormalImportApi
      && result?.readiness.ready
      && stage === "ready",
  );

  async function handleFileSelected(nextFile?: File) {
    clearSelection(false);
    if (!nextFile) return;
    const requestId = ++readRequestRef.current;

    const nextFormat = importFormatForFileName(nextFile.name);
    setFile(nextFile);
    setFormat(nextFormat);
    setStage("parsing");
    if (!nextFormat) {
      const issue: ImportIssue = {
        id: "file-unsupported-format",
        severity: "error",
        blocking: true,
        code: "IMPORT_UNSUPPORTED_FORMAT",
        message: "当前只支持 JSON 或 TSV/TXT 镜头批量导入文件。",
      };
      setResult(failedImportResult(issue));
      setError(issue);
      setStage("failed");
      return;
    }

    try {
      const nextContent = normalizeImportContent(await nextFile.text());
      if (requestId !== readRequestRef.current) return;
      if (!nextContent.trim()) {
        const issue: ImportIssue = {
          id: "file-empty",
          severity: "error",
          blocking: true,
          code: "BULK_IMPORT_EMPTY_INPUT",
          message: "导入文件为空，至少需要一行镜头。",
        };
        setResult(failedImportResult(issue));
        setError(issue);
        setStage("failed");
        return;
      }
      setContent(nextContent);
      setStage("parsed");
    } catch (readError: unknown) {
      if (requestId !== readRequestRef.current) return;
      const issue = importIssueFromError(readError);
      issue.code = "IMPORT_FILE_READ_FAILED";
      issue.message = "读取导入文件失败，请检查文件权限后重试。";
      setResult(failedImportResult(issue));
      setError(readError);
      setStage("failed");
    }
  }

  async function runDryRun() {
    if (!format || !content || busy) return;
    setStage("validating");
    setResult(undefined);
    setError(undefined);
    try {
      const preview = await previewShotBulkImport({ projectId, format, content });
      setResult(buildImportDryRunResult(preview));
      setStage(preview.invalid === 0 ? "ready" : "validated");
    } catch (previewError: unknown) {
      const issue = importIssueFromError(previewError);
      setResult(failedImportResult(issue));
      setError(previewError);
      setStage("failed");
    }
  }

  async function executeImport() {
    if (!canExecuteImport || !format || !result) return;
    const createCount = result.summary.createCount ?? 0;
    if (!window.confirm(`确认导入 ${createCount} 个镜头？预检通过后才会写入当前项目。`)) return;
    setStage("executing");
    setError(undefined);
    try {
      const imported = await commitShotBulkImport({ projectId, format, content });
      setExecutionResult(imported);
      setStage("completed");
      await onImported?.();
    } catch (executionError: unknown) {
      setResult(failedImportResult(importIssueFromError(executionError)));
      setError(executionError);
      setStage("failed");
    }
  }

  function clearSelection(clearInput = true) {
    readRequestRef.current += 1;
    setFile(undefined);
    setFormat(undefined);
    setContent("");
    setResult(undefined);
    setExecutionResult(undefined);
    setError(undefined);
    setStage("idle");
    if (clearInput && fileInputRef.current) fileInputRef.current.value = "";
  }

  return (
    <section className="workspace-panel project-import-workspace" aria-busy={busy} aria-label="批量导入预检工作区">
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">项目工具</span>
          <h2>批量导入 / 导入预检</h2>
          <p className="section-description">先读取、检查并预览镜头文件；选择文件不会修改项目数据。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onClose} disabled={busy}>返回项目指挥中心</button>
      </div>

      <section className="project-import-file-card" aria-labelledby="project-import-file-title">
        <div className="project-import-card-heading">
          <div>
            <span className="section-label">1 · 选择文件</span>
            <h3 id="project-import-file-title">读取导入文件</h3>
          </div>
          {file && <span className={`project-import-stage project-import-stage-${stage}`}>{stageLabel(stage)}</span>}
        </div>
        <label className="project-import-file-picker" htmlFor="project-import-file">
          <span>选择 JSON / TSV 文件</span>
          <input
            ref={fileInputRef}
            id="project-import-file"
            type="file"
            accept=".json,.tsv,.txt,application/json,text/tab-separated-values,text/plain"
            onChange={(event) => void handleFileSelected(event.target.files?.[0])}
            disabled={busy}
          />
        </label>
        <p className="project-import-help">当前真实支持范围：JSON 或 TSV/TXT 镜头批量导入。不会创建或修改 Episode、Scene、Character 等其他实体。</p>
        {file && (
          <dl className="project-import-file-meta">
            <div><dt>文件名</dt><dd>{file.name}</dd></div>
            <div><dt>格式</dt><dd>{format ? format.toUpperCase() : "不支持"}</dd></div>
            <div><dt>类型</dt><dd>{file.type || "未提供（按扩展名识别）"}</dd></div>
            <div><dt>大小</dt><dd>{formatBytes(file.size)}</dd></div>
          </dl>
        )}
        <div className="project-import-actions">
          <button type="button" className="primary-action" onClick={() => void runDryRun()} disabled={busy || stage !== "parsed" || !content}>
            {stage === "validating" ? "正在运行预检…" : "运行预检"}
          </button>
          <button type="button" className="quiet-button" onClick={() => clearSelection()} disabled={busy || !file}>清除并重新选择</button>
        </div>
      </section>

      {error !== undefined && stage !== "failed" && <UiErrorNotice error={error} />}
      {stage === "failed" && result && <ProjectImportIssueSummary result={result} error={error} />}

      {result && (stage === "ready" || stage === "validated") && (
        <>
          <ProjectImportSummary result={result} />
          <ProjectImportRows rows={result.rows ?? []} />
          <ProjectImportIssues issues={result.issues} />
          <section className="project-import-dry-run-card" aria-labelledby="project-import-dry-run-title">
            <div className="project-import-card-heading">
              <div>
                <span className="section-label">3 · Dry-Run</span>
                <h3 id="project-import-dry-run-title">预计变化</h3>
              </div>
              <strong className={result.readiness.ready ? "project-import-ready" : "project-import-blocked"}>
                {result.readiness.ready ? "可以确认导入" : "存在阻塞项"}
              </strong>
            </div>
            {result.readiness.ready ? (
              <p>预计新增 {result.summary.createCount ?? 0} 个镜头。现有导入接口为 CREATE ONLY，不会更新或删除已有数据。</p>
            ) : (
              <p>预检未通过；正式确认不会写入任何项目数据。修复文件后重新选择并运行预检。</p>
            )}
            <div className="project-import-formal-capability">
              <strong>正式导入能力</strong>
              {hasFormalImportApi ? (
                <span>使用现有镜头批量导入 API，需再次确认后才写入。</span>
              ) : (
                <span>当前版本仅支持导入预检，尚未提供正式写入能力。</span>
              )}
            </div>
            <button
              type="button"
              className="primary-action"
              onClick={() => void executeImport()}
              disabled={!canExecuteImport}
              title={hasFormalImportApi ? undefined : "当前版本尚未提供正式导入 API"}
            >
              确认导入
            </button>
          </section>
        </>
      )}

      {stage === "completed" && executionResult && (
        <section className="project-import-success" role="status" aria-label="导入完成">
          <strong>导入完成</strong>
          <p>已写入 {executionResult.created.length} 个镜头；本次没有自动生成任务。</p>
          <button type="button" className="quiet-button" onClick={() => clearSelection()}>继续导入其他文件</button>
        </section>
      )}
    </section>
  );
}

function ProjectImportSummary({ result }: { result: ImportDryRunResult }) {
  const { readiness } = result;
  return (
    <section className="project-import-summary" role="status" aria-label="导入文件摘要">
      <div className="project-import-card-heading"><div><span className="section-label">2 · Parse / Validate</span><h3>导入文件摘要</h3></div><strong>{readiness.ready ? "预检通过" : "预检阻塞"}</strong></div>
      <div className="project-import-summary-grid">
        <span><small>记录总数</small><strong>{readiness.totalRecords}</strong></span>
        <span><small>可识别实体</small><strong>{readiness.validRecords}</strong></span>
        <span><small>无法识别 / 阻塞</small><strong>{readiness.invalidRecords}</strong></span>
        <span><small>错误数量</small><strong>{readiness.errorCount}</strong></span>
        <span><small>警告数量</small><strong>{readiness.warningCount}</strong></span>
        <span><small>阻塞错误</small><strong>{readiness.blockingCount}</strong></span>
      </div>
    </section>
  );
}

function ProjectImportIssueSummary({ result, error }: { result: ImportDryRunResult; error?: unknown }) {
  return (
    <>
      {error && <UiErrorNotice error={error} />}
      <ProjectImportSummary result={result} />
      <ProjectImportRows rows={result.rows ?? []} />
      <ProjectImportIssues issues={result.issues} />
    </>
  );
}

function ProjectImportIssues({ issues }: { issues: ImportIssue[] }) {
  if (!issues.length) return <p className="project-import-no-issues">未发现错误或警告。</p>;
  return (
    <section className="project-import-issues" aria-labelledby="project-import-issues-title">
      <div className="project-import-card-heading"><div><span className="section-label">问题</span><h3 id="project-import-issues-title">问题列表</h3></div><strong>{issues.length} 项</strong></div>
      <ul>
        {issues.map((issue) => <li key={issue.id} className={`project-import-issue project-import-issue-${issue.severity}`}>
          <div><strong>{issue.severity === "error" ? "ERROR" : issue.severity === "warning" ? "WARNING" : "INFO"}</strong>{issue.code && <code>{issue.code}</code>}{issue.entityType && <span>{issue.entityType}</span>}{issue.entityId && <span>{issue.entityId}</span>}{issue.row !== undefined && <span>第 {issue.row} 行</span>}</div>
          <p>{issue.message}</p>
        </li>)}
      </ul>
    </section>
  );
}

function ProjectImportRows({ rows }: { rows: NonNullable<ImportDryRunResult["rows"]> }) {
  if (!rows.length) return null;
  const visibleRows = rows.slice(0, 20);
  return (
    <section className="project-import-rows" aria-labelledby="project-import-rows-title">
      <div className="project-import-card-heading"><div><span className="section-label">内容概览</span><h3 id="project-import-rows-title">记录预览</h3></div><strong>{rows.length} 条</strong></div>
      <div className="project-import-table-wrap">
        <table aria-label="导入记录预览">
          <thead><tr><th scope="col">行</th><th scope="col">镜头名称</th><th scope="col">描述</th><th scope="col">图片 Prompt</th><th scope="col">视频 Prompt</th><th scope="col">状态</th></tr></thead>
          <tbody>{visibleRows.map((row) => <tr key={row.rowNumber}>
            <td>{row.rowNumber}</td>
            <td>{row.name || "—"}</td>
            <td>{row.description || "—"}</td>
            <td>{row.imagePrompt || "—"}</td>
            <td>{row.videoPrompt || "—"}</td>
            <td>{row.errors.length ? "阻塞" : row.warnings.length ? "警告" : "可导入"}</td>
          </tr>)}</tbody>
        </table>
      </div>
      {rows.length > visibleRows.length && <p className="project-import-help">仅展示前 {visibleRows.length} 条，预检统计覆盖全部 {rows.length} 条记录。</p>}
    </section>
  );
}

function stageLabel(stage: ImportStage): string {
  switch (stage) {
    case "parsing": return "正在读取";
    case "parsed": return "已读取，待预检";
    case "validating": return "正在预检";
    case "validated": return "预检阻塞";
    case "ready": return "预检通过";
    case "executing": return "正在写入";
    case "completed": return "已完成";
    case "failed": return "预检失败";
    default: return "已选择";
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}
