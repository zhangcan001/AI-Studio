import { useEffect, useRef, useState } from "react";
import {
  confirmExternalProductionHandoff,
  getExternalProductionHandoffMappings,
  listExternalProductionHandoffs,
  previewExternalProductionHandoff,
} from "../../services/tauriClient";
import type {
  ExternalProductionHandoffHistoryItem,
  ExternalProductionHandoffMapping,
  ExternalProductionHandoffPreview,
} from "../../types/externalProductionHandoff";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";

interface Props {
  projectId: string;
  onBack: () => void;
  onImported?: () => void | Promise<void>;
  onOpenStructure?: () => void;
}

export function ExternalAgentHandoffPanel({ projectId, onBack, onImported, onOpenStructure }: Props) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [content, setContent] = useState("");
  const [fileName, setFileName] = useState("");
  const [preview, setPreview] = useState<ExternalProductionHandoffPreview>();
  const [history, setHistory] = useState<ExternalProductionHandoffHistoryItem[]>([]);
  const [selectedHandoffId, setSelectedHandoffId] = useState<string>();
  const [mappings, setMappings] = useState<ExternalProductionHandoffMapping[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>();
  const [completed, setCompleted] = useState(false);

  useEffect(() => {
    setContent("");
    setFileName("");
    setPreview(undefined);
    setError(undefined);
    setCompleted(false);
    setSelectedHandoffId(undefined);
    setMappings([]);
    void listExternalProductionHandoffs(projectId)
      .then(setHistory)
      .catch(setError);
  }, [projectId]);

  async function selectFile(file?: File) {
    if (!file) return;
    try {
      setError(undefined);
      setFileName(file.name);
      setContent((await file.text()).replace(/^\uFEFF/, ""));
      setPreview(undefined);
      setCompleted(false);
    } catch (readError) {
      setError(readError);
    }
  }

  async function runPreview() {
    if (!content.trim() || busy) return;
    setBusy(true);
    setError(undefined);
    setCompleted(false);
    try {
      setPreview(await previewExternalProductionHandoff({ projectId, content }));
    } catch (previewError) {
      setPreview(undefined);
      setError(previewError);
    } finally {
      setBusy(false);
    }
  }

  async function confirmImport() {
    if (!preview || preview.errors.length > 0 || busy) return;
    if (!window.confirm("确认将该外部 Agent 交接写入当前项目？此操作只创建正式生产结构，不会自动入队或生成任务。")) return;
    setBusy(true);
    setError(undefined);
    try {
      await confirmExternalProductionHandoff({
        projectId,
        content,
        expectedDocumentSha256: preview.documentSha256,
      });
      setCompleted(true);
      setHistory(await listExternalProductionHandoffs(projectId));
      await onImported?.();
    } catch (confirmError) {
      setError(confirmError);
    } finally {
      setBusy(false);
    }
  }

  async function showMappings(handoffId: string) {
    setError(undefined);
    try {
      setSelectedHandoffId(handoffId);
      setMappings(await getExternalProductionHandoffMappings(projectId, handoffId));
    } catch (mappingError) {
      setError(mappingError);
    }
  }

  return (
    <section className="workspace-panel project-import-workspace" aria-busy={busy} aria-label="External Agent Handoff 工作区">
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">项目工具 · External Agent</span>
          <h2>External Agent Production Handoff</h2>
          <p className="section-description">读取严格的 ProductionHandoffV1 文档，先预检，再由你明确确认写入正式生产结构。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onBack} disabled={busy}>返回镜头批量导入</button>
      </div>

      <section className="project-import-file-card" aria-labelledby="external-handoff-file-title">
        <div className="project-import-card-heading">
          <div>
            <span className="section-label">1 · 选择交接文档</span>
            <h3 id="external-handoff-file-title">ProductionHandoffV1 JSON</h3>
          </div>
          {fileName && <span>{fileName}</span>}
        </div>
        <label className="project-import-file-picker" htmlFor="external-handoff-file">
          <span>选择 JSON 文件</span>
          <input
            ref={fileInputRef}
            id="external-handoff-file"
            type="file"
            accept=".json,application/json"
            onChange={(event) => void selectFile(event.target.files?.[0])}
            disabled={busy}
          />
        </label>
        <label className="external-handoff-editor-label" htmlFor="external-handoff-content">或粘贴交接 JSON</label>
        <textarea
          id="external-handoff-content"
          className="external-handoff-editor"
          value={content}
          onChange={(event) => {
            setContent(event.target.value);
            setPreview(undefined);
            setCompleted(false);
          }}
          placeholder='{"schemaVersion":1,"projectId":"...","source":{"agent":"..."},"series":[]}'
          rows={10}
          disabled={busy}
        />
        <p className="project-import-help">预检是只读的；写入会创建 Series / Episode / Scene / Shot、提示词、阶段配置、已有素材引用和来源映射，不会创建 Queue Task、Comfy 任务或自动执行。</p>
        <div className="project-import-actions">
          <button type="button" className="primary-action" onClick={() => void runPreview()} disabled={busy || !content.trim()}>
            {busy ? "处理中…" : "运行交接预检"}
          </button>
          <button type="button" className="quiet-button" onClick={() => { setContent(""); setFileName(""); setPreview(undefined); setCompleted(false); if (fileInputRef.current) fileInputRef.current.value = ""; }} disabled={busy || !content}>
            清除
          </button>
        </div>
      </section>

      {error !== undefined && <UiErrorNotice error={error} />}
      {preview && (
        <section className="project-import-dry-run-card" aria-label="External Agent Handoff 预检结果">
          <div className="project-import-card-heading">
            <div><span className="section-label">2 · Preview</span><h3>预检结果</h3></div>
            <strong className={preview.errors.length === 0 ? "project-import-ready" : "project-import-blocked"}>
              {preview.errors.length === 0 ? "可以确认" : "存在阻塞项"}
            </strong>
          </div>
          <div className="project-import-summary-grid">
            <span><small>Series</small><strong>{preview.seriesCount}</strong></span>
            <span><small>Episode</small><strong>{preview.episodeCount}</strong></span>
            <span><small>Scene</small><strong>{preview.sceneCount}</strong></span>
            <span><small>Shot</small><strong>{preview.shotCount}</strong></span>
            <span><small>SHA256</small><strong title={preview.documentSha256}>{preview.documentSha256.slice(0, 12)}…</strong></span>
          </div>
          {preview.replay.status !== "NEW" && <p role="status">幂等状态：{preview.replay.status}，不会重复创建正式实体。</p>}
          {(preview.errors.length > 0 || preview.warnings.length > 0) && (
            <ul className="project-import-issues">
              {[...preview.errors, ...preview.warnings].map((issue, index) => <li key={`${issue.code}-${index}`}><strong>{issue.code}</strong>：{issue.message}</li>)}
            </ul>
          )}
          <p>预计写入：{preview.writePlan.createsSeries} 个 Series、{preview.writePlan.createsEpisodes} 个 Episode、{preview.writePlan.createsScenes} 个 Scene、{preview.writePlan.createsShots} 个 Shot。</p>
          <button type="button" className="primary-action" onClick={() => void confirmImport()} disabled={busy || completed || preview.errors.length > 0}>
            {completed ? "已确认写入" : "明确确认并写入"}
          </button>
          {completed && onOpenStructure && <button type="button" className="quiet-button" onClick={onOpenStructure}>打开项目结构</button>}
        </section>
      )}

      <section className="project-import-dry-run-card" aria-label="External Agent Handoff 历史">
        <div className="project-import-card-heading"><div><span className="section-label">3 · History</span><h3>交接历史</h3></div></div>
        {history.length === 0 ? <p>当前项目暂无外部交接记录。</p> : <ul className="project-import-issues">{history.map((item) => <li key={item.id}><div><strong>{item.sourceAgent}</strong>{item.sourceRevision ? ` @ ${item.sourceRevision}` : ""} · {item.documentSha256.slice(0, 12)}… · {new Date(item.importedAt).toLocaleString()}</div><button type="button" className="quiet-button" onClick={() => void showMappings(item.id)} disabled={busy}>查看映射</button>{selectedHandoffId === item.id && <ul><li>{mappings.length === 0 ? "没有实体映射" : `${mappings.length} 个实体映射已加载`}</li>{mappings.map((mapping) => <li key={`${mapping.entityKind}-${mapping.externalId}`}><code>{mapping.entityKind}</code> {mapping.externalId} → {mapping.formalEntityId}</li>)}</ul>}</li>)}</ul>}
      </section>
    </section>
  );
}
