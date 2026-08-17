import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toUserMessage } from "../../i18n/errorMessages";

export type ShotBulkImportFormat = "tsv" | "json";

type ShotBulkImportIssue = string | { message: string };

export interface ShotBulkImportRowPreview {
  rowNumber: number;
  name: string;
  description: string;
  imagePrompt: string;
  videoPrompt: string;
  errors: ShotBulkImportIssue[];
  warnings: ShotBulkImportIssue[];
}

export interface ShotBulkImportPreview {
  total: number;
  valid: number;
  invalid: number;
  warnings: number;
  rows: ShotBulkImportRowPreview[];
}

export interface ShotBulkImportRequest {
  projectId: string;
  format: ShotBulkImportFormat;
  content: string;
}

export interface ShotBulkImportPanelProps {
  projectId: string;
  onImported?: () => void | Promise<void>;
  onCancel?: () => void;
}

export function previewShotBulkImport(request: ShotBulkImportRequest): Promise<ShotBulkImportPreview> {
  return invoke<ShotBulkImportPreview>("preview_shot_bulk_import", { request });
}

export function commitShotBulkImport(request: ShotBulkImportRequest): Promise<unknown> {
  return invoke("commit_shot_bulk_import", { request });
}

export function shotBulkImportRowClassName(row: ShotBulkImportRowPreview): string {
  return row.errors.length > 0 ? "shot-batch-row-blocked" : "";
}

const EMPTY_COUNTS = { total: 0, valid: 0, invalid: 0, warnings: 0 };

export function ShotBulkImportPanel({ projectId, onImported, onCancel }: ShotBulkImportPanelProps) {
  const [format, setFormat] = useState<ShotBulkImportFormat>("tsv");
  const [content, setContent] = useState("");
  const [preview, setPreview] = useState<ShotBulkImportPreview>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();

  const counts = preview ?? EMPTY_COUNTS;
  const canConfirm = Boolean(preview && preview.total > 0 && preview.valid > 0 && preview.invalid === 0);

  function resetPreview(nextContent = content) {
    setContent(nextContent);
    setPreview(undefined);
    setError(undefined);
    setNotice(undefined);
  }

  function selectFormat(nextFormat: ShotBulkImportFormat) {
    if (nextFormat === format) return;
    setFormat(nextFormat);
    resetPreview();
  }

  async function checkImport() {
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      setPreview(await previewShotBulkImport({ projectId, format, content }));
    } catch (value: unknown) {
      setPreview(undefined);
      setError(toUserMessage(value));
    } finally {
      setBusy(false);
    }
  }

  async function confirmImport() {
    if (!preview || !canConfirm) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    const importedCount = preview.valid;
    try {
      await commitShotBulkImport({ projectId, format, content });
      setContent("");
      setPreview(undefined);
      setNotice(`已导入 ${importedCount} 个 Shot。`);
      await onImported?.();
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally {
      setBusy(false);
    }
  }

  function cancelImport() {
    setContent("");
    setPreview(undefined);
    setError(undefined);
    setNotice(undefined);
    onCancel?.();
  }

  return (
    <section className="shot-batch-panel shot-batch-panel-open" aria-label="批量导入镜头">
      <div className="shot-batch-panel-heading">
        <div>
          <span className="section-label">项目生产</span>
          <h3>批量导入镜头</h3>
          <p className="shot-inline-note">先检查全部行，再一次性确认导入；检查阶段不会写入项目。</p>
        </div>
        <button type="button" className="quiet-button" onClick={cancelImport} disabled={busy}>
          取消
        </button>
      </div>

      <div className="shot-batch-stage-tabs" role="tablist" aria-label="批量导入格式">
        <button type="button" role="tab" aria-selected={format === "tsv"} className={format === "tsv" ? "active" : ""} onClick={() => selectFormat("tsv")} disabled={busy}>
          TSV
        </button>
        <button type="button" role="tab" aria-selected={format === "json"} className={format === "json" ? "active" : ""} onClick={() => selectFormat("json")} disabled={busy}>
          JSON
        </button>
      </div>

      <label>
        <span className="section-label">导入内容</span>
        <textarea
          value={content}
          onChange={(event) => resetPreview(event.target.value)}
          placeholder={format === "tsv" ? "镜头名称\t镜头描述\t图片提示词\t视频提示词" : '{"schemaVersion":1,"shots":[]}' }
          rows={8}
          aria-label={`${format.toUpperCase()} 导入内容`}
          disabled={busy}
        />
      </label>

      <div className="shot-batch-selection-bar" aria-label="导入检查计数">
        <span>总行数：<strong>{counts.total}</strong></span>
        <span>可导入：<strong>{counts.valid}</strong></span>
        <span>错误：<strong>{counts.invalid}</strong></span>
        <span>警告：<strong>{counts.warnings}</strong></span>
        <button type="button" className="quiet-button" onClick={() => void checkImport()} disabled={busy}>
          {busy ? "正在检查…" : "检查"}
        </button>
        <button type="button" className="shot-primary-action" onClick={() => void confirmImport()} disabled={busy || !canConfirm}>
          {busy ? "正在导入…" : "确认导入"}
        </button>
      </div>

      {error && <p className="shot-recent-failure" role="alert">{error}</p>}
      {notice && <p className="shot-inline-note" role="status">{notice}</p>}

      {preview ? (
        <div className="shot-batch-table-wrap">
          <table className="shot-batch-table" aria-label="批量导入预览">
            <thead>
              <tr>
                <th>行</th>
                <th>镜头名称</th>
                <th>镜头描述</th>
                <th>图片提示词</th>
                <th>视频提示词</th>
                <th>校验结果</th>
              </tr>
            </thead>
            <tbody>
              {preview.rows.map((row) => (
                <tr key={row.rowNumber} className={shotBulkImportRowClassName(row)} aria-invalid={row.errors.length > 0}>
                  <td>{row.rowNumber}</td>
                  <td><strong>{row.name || "—"}</strong></td>
                  <td>{row.description || "—"}</td>
                  <td>{row.imagePrompt || "—"}</td>
                  <td>{row.videoPrompt || "—"}</td>
                  <td>
                    {row.errors.length > 0 && <ul className="shot-batch-reasons">{row.errors.map((issue, index) => <li key={index}>{issueMessage(issue)}</li>)}</ul>}
                    {row.warnings.length > 0 && <ul className="shot-frozen-config-warning">{row.warnings.map((issue, index) => <li key={index}>{issueMessage(issue)}</li>)}</ul>}
                    {!row.errors.length && !row.warnings.length && <span className="shot-batch-ready">可导入</span>}
                  </td>
                </tr>
              ))}
              {!preview.rows.length && <tr><td colSpan={6}><p className="empty-state">没有可预览的镜头行。</p></td></tr>}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="empty-state">输入 TSV 或 JSON 后点击“检查”，这里会显示逐行预览。</p>
      )}
    </section>
  );
}

function issueMessage(issue: ShotBulkImportIssue): string {
  return typeof issue === "string" ? issue : issue.message;
}
