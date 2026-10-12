import { useEffect, useMemo, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { invalidateVideoInputs } from "../../product/videoInputInvalidation";
import { normalizeProductError } from "../../product/errors";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotView } from "../../types/shot";
import type { VideoInputAsset } from "../../types/shotVideoInput";
import {
  BATCH_BIND_KEY_LABELS, type BatchBindKey, assetAllowedForBindKey, groupAssignmentsByShot,
  mergeShotBindings, parseBatchBindTable, resolveBatchBindRows, shotsReadyForBatchBind,
} from "./shotBatchAssetBind";

export interface ShotBatchAssetBindPanelProps {
  projectId: string; shots: ShotView[]; catalog: RecipeViewModel[];
  onBound?: () => void | Promise<void>; onCancel?: () => void;
}
type MatrixDraft = Record<string, Partial<Record<BatchBindKey, string>>>;
interface SourceAssetOption { id: string; title: string; category: string; mediaKind: "image" | "video" | "audio" }
function mediaKindForKey(key: BatchBindKey): "image" | "video" | "audio" {
  if (key === "reference_videos") return "video";
  if (key === "reference_audios") return "audio";
  return "image";
}

/** Main path: external Library assets → multi-shot formal ShotVideoInput slots via OCC. No Task/Queue. */
export function ShotBatchAssetBindPanel({ projectId, shots, catalog, onBound, onCancel }: ShotBatchAssetBindPanelProps) {
  const [mode, setMode] = useState<"matrix" | "tsv">("matrix");
  const [content, setContent] = useState("");
  const [matrix, setMatrix] = useState<MatrixDraft>({});
  const [sourceAssets, setSourceAssets] = useState<SourceAssetOption[]>([]);
  const [busy, setBusy] = useState(false);
  const [checkingTsv, setCheckingTsv] = useState(false);
  const [loadingAssets, setLoadingAssets] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [previewLines, setPreviewLines] = useState<ReturnType<typeof resolveBatchBindRows>>();
  const lock = useRef(false);
  const validationLock = useRef(false);
  const validationVersion = useRef(0);
  const liveProject = useRef(projectId); liveProject.current = projectId;
  useEffect(() => {
    validationVersion.current += 1;
    setPreviewLines(undefined);
    return () => { validationVersion.current += 1; };
  }, [projectId, shots, catalog]);
  const readyShots = useMemo(() => shotsReadyForBatchBind(shots, catalog), [shots, catalog]);

  useEffect(() => {
    let cancelled = false;
    setLoadingAssets(true); setError(undefined);
    (async () => {
      try {
        const collected: SourceAssetOption[] = [];
        let cursor: import("../../product/libraryTypes").LibraryCursor | null = null;
        for (let page = 0; page < 20; page += 1) {
          const list = await productClient.library.list(projectId, {
            category: "media", keyword: null, favoriteOnly: false, tagId: null, sourceKind: "source", cursor, limit: 30,
          });
          if (cancelled || liveProject.current !== projectId) return;
          for (const item of list.items) {
            if (item.resourceRef.kind !== "asset") continue;
            const mediaKind = item.subtype === "video" ? "video" : item.subtype === "audio" ? "audio" : item.subtype === "image" ? "image" : null;
            if (!mediaKind) continue;
            collected.push({
              id: item.resourceRef.id, title: item.title, mediaKind,
              category: mediaKind === "video" ? "source_video" : mediaKind === "audio" ? "source_audio" : "source_image",
            });
          }
          cursor = list.nextCursor; if (!cursor) break;
        }
        if (!cancelled && liveProject.current === projectId) setSourceAssets(collected);
      } catch (value: unknown) {
        if (!cancelled && liveProject.current === projectId) { setError(normalizeProductError(value).message); setSourceAssets([]); }
      } finally {
        if (!cancelled && liveProject.current === projectId) setLoadingAssets(false);
      }
    })();
    return () => { cancelled = true; };
  }, [projectId]);

  function setMatrixValue(shotId: string, key: BatchBindKey, assetId: string) {
    setMatrix((current) => ({ ...current, [shotId]: { ...current[shotId], [key]: assetId } }));
    setNotice(undefined); setError(undefined);
  }

  /** CapCut/spreadsheet-style: copy the first filled cell in a column into every eligible row. */
  function fillColumn(key: BatchBindKey) {
    let seed = "";
    for (const entry of readyShots) {
      if (!entry.slots.includes(key)) continue;
      const value = (matrix[entry.shot.id]?.[key] ?? "").trim();
      if (value) { seed = value; break; }
    }
    if (!seed) {
      setError(`请先在「${BATCH_BIND_KEY_LABELS[key]}」列选中一格外部素材，再按列填充。`);
      return;
    }
    setMatrix((current) => {
      const next = { ...current };
      for (const entry of readyShots) {
        if (!entry.slots.includes(key)) continue;
        next[entry.shot.id] = { ...next[entry.shot.id], [key]: seed };
      }
      return next;
    });
    setError(undefined);
    setNotice(`已用首个已选素材填充「${BATCH_BIND_KEY_LABELS[key]}」列（仅写入草稿，尚未 OCC 保存）。`);
  }

  function clearMatrix() {
    setMatrix({});
    setNotice(undefined);
    setError(undefined);
  }

  /** CapCut fill-down: copy this shot's filled slots onto later shots that expose the same keys. */
  function copyRowDown(shotId: string) {
    const source = readyShots.find((entry) => entry.shot.id === shotId);
    if (!source) return;
    const draft = matrix[shotId] ?? {};
    const payload = Object.fromEntries(
      source.slots
        .map((key) => [key, (draft[key] ?? "").trim()] as const)
        .filter(([, value]) => value),
    ) as Partial<Record<BatchBindKey, string>>;
    if (!Object.keys(payload).length) {
      setError("请先在本行选好至少一个槽位，再复制到下方镜头。");
      return;
    }
    const startIndex = readyShots.findIndex((entry) => entry.shot.id === shotId);
    if (startIndex < 0 || startIndex >= readyShots.length - 1) {
      setError("下方没有可接收的镜头。");
      return;
    }
    const targets = readyShots.slice(startIndex + 1).filter((entry) =>
      entry.slots.some((key) => Boolean(payload[key])),
    );
    if (!targets.length) {
      setError("下方镜头没有可匹配的正式槽，未复制。");
      return;
    }
    setMatrix((current) => {
      const next = { ...current };
      for (const entry of targets) {
        const row = { ...next[entry.shot.id] };
        for (const key of entry.slots) {
          const value = payload[key];
          if (value) row[key] = value;
        }
        next[entry.shot.id] = row;
      }
      return next;
    });
    setError(undefined);
    setNotice(`已把镜头 ${source.shot.ordinal + 1} 的已选槽复制到下方 ${targets.length} 个镜头（草稿，尚未 OCC 保存）。`);
  }

  function rowFillStats(entry: (typeof readyShots)[number]) {
    const draft = matrix[entry.shot.id] ?? {};
    const filled = entry.slots.filter((key) => (draft[key] ?? "").trim()).length;
    return { filled, total: entry.slots.length };
  }

  function assetsForKey(key: BatchBindKey) {
    return sourceAssets.filter((asset) => asset.mediaKind === mediaKindForKey(key));
  }

  function matrixAssignments() {
    const rows: Array<{ line: number; shotId: string; shotName: string; shotOrdinal: number; inputKey: BatchBindKey; assetId: string; selectionRef: string; workflowVersionId: string; recipeId: string; errors: string[] }> = [];
    let line = 1;
    for (const entry of readyShots) {
      const draft = matrix[entry.shot.id] ?? {};
      for (const key of entry.slots) {
        const assetId = (draft[key] ?? "").trim();
        if (!assetId) continue;
        const option = sourceAssets.find((item) => item.id === assetId);
        const errors: string[] = [];
        if (!option) errors.push("请选择外部导入素材");
        else if (!assetAllowedForBindKey(key, option.category) || option.mediaKind !== mediaKindForKey(key)) {
          errors.push(`${BATCH_BIND_KEY_LABELS[key]}仅接受匹配类型的外部导入素材`);
        }
        rows.push({
          line: line++, shotId: entry.shot.id, shotName: entry.shot.name, shotOrdinal: entry.shot.ordinal + 1,
          inputKey: key, assetId, selectionRef: entry.selectionRef,
          workflowVersionId: entry.recipe.workflowVersionId, recipeId: entry.recipe.recipeId, errors,
        });
      }
    }
    return rows;
  }

  /** Preview and confirmation use the same project-scoped authoritative Library lookup. */
  async function validateTsvAssets(
    assignments: ReturnType<typeof resolveBatchBindRows>,
    version: number,
  ): Promise<ReturnType<typeof resolveBatchBindRows> | undefined> {
    const validated: ReturnType<typeof resolveBatchBindRows> = [];
    for (const row of assignments) {
      if (validationVersion.current !== version || liveProject.current !== projectId) return undefined;
      const errors = [...row.errors];
      if (!errors.length) {
        try {
          const detail = await productClient.library.get(projectId, { kind: "asset", id: row.assetId });
          if (validationVersion.current !== version || liveProject.current !== projectId) return undefined;
          if (detail.kind !== "asset" || detail.asset.id !== row.assetId) {
            errors.push("素材不存在");
          } else if (!assetAllowedForBindKey(row.inputKey, detail.asset.category, detail.asset.sourceTaskId)) {
            errors.push(`${BATCH_BIND_KEY_LABELS[row.inputKey]}只接受匹配类型的外部导入素材`);
          } else if (detail.asset.hasExternalImportReceipt !== true) {
            errors.push("素材缺少有效的外部导入收据，不能写入正式输入");
          }
        } catch (value: unknown) {
          if (validationVersion.current !== version || liveProject.current !== projectId) return undefined;
          errors.push(normalizeProductError(value).message);
        }
      }
      validated.push({ ...row, errors });
    }
    return validated;
  }

  async function checkTsv() {
    if (lock.current || validationLock.current) return;
    const version = ++validationVersion.current;
    validationLock.current = true;
    setCheckingTsv(true); setPreviewLines(undefined);
    setError(undefined); setNotice(undefined);
    try {
      const parsed = parseBatchBindTable(content);
      if (!parsed.length) {
        if (validationVersion.current === version) { setPreviewLines([]); setError("没有可检查的绑定行。"); }
        return;
      }
      const validated = await validateTsvAssets(resolveBatchBindRows(parsed, shots, catalog), version);
      if (validated && validationVersion.current === version) setPreviewLines(validated);
    } finally {
      validationLock.current = false;
      setCheckingTsv(false);
    }
  }

  async function applyPlans(assignments: ReturnType<typeof matrixAssignments>) {
    if (lock.current) return;
    const blocked = assignments.filter((row) => row.errors.length);
    if (blocked.length) { setError(`有 ${blocked.length} 行无法绑定，请先修正。`); return; }
    const plans = groupAssignmentsByShot(assignments);
    if (!plans.length) { setError("没有需要写入的绑定。"); return; }
    lock.current = true; setBusy(true); setError(undefined); setNotice(undefined);
    let savedShots = 0;
    try {
      for (const plan of plans) {
        if (liveProject.current !== projectId) return;
        const selection = { projectId, shotId: plan.shotId, selectionRef: plan.selectionRef };
        const view = await productClient.creation.inputsGet(selection);
        if (liveProject.current !== projectId) return;
        if (view.selectionRef !== selection.selectionRef) throw new Error(`镜头 ${plan.shotOrdinal} 的生成器身份不匹配，请刷新后重试。`);
        let inputs: VideoInputAsset[] = view.inputs;
        for (const assignment of plan.assignments) {
          const detail = await productClient.library.get(projectId, { kind: "asset", id: assignment.assetId });
          if (liveProject.current !== projectId) return;
          if (detail.kind !== "asset") throw new Error(`镜头 ${plan.shotOrdinal} 素材 ${assignment.assetId} 不可用。`);
          if (!assetAllowedForBindKey(assignment.inputKey, detail.asset.category, detail.asset.sourceTaskId)) {
            throw new Error(`镜头 ${plan.shotOrdinal} 的 ${BATCH_BIND_KEY_LABELS[assignment.inputKey]} 只接受外部导入素材。`);
          }
          if (detail.asset.hasExternalImportReceipt !== true) {
            throw new Error(`镜头 ${plan.shotOrdinal} 素材缺少有效的外部导入收据，不能写入正式输入。`);
          }
          const merged = mergeShotBindings(inputs, assignment.inputKey, assignment.assetId);
          if (merged.error) throw new Error(merged.error);
          inputs = merged.inputs;
        }
        await productClient.creation.inputsSave({ selection, expected: view.token, inputs });
        savedShots += 1;
      }
      setNotice(`已写入 ${savedShots} 个镜头的正式视频输入（OCC）。未创建任务，也未启动生产队列。`);
      setMatrix({}); setContent(""); setPreviewLines(undefined);
      invalidateVideoInputs(projectId);
      await onBound?.();
    } catch (value: unknown) {
      setError(value instanceof Error ? value.message : normalizeProductError(value).message);
    } finally {
      lock.current = false; setBusy(false);
    }
  }

  async function confirmMatrix() { await applyPlans(matrixAssignments()); }
  async function confirmTsv() {
    if (!previewLines || previewLines.length === 0 || previewLines.some((row) => row.errors.length)) {
      setError("请先通过表格校验，修正所有错误行。"); return;
    }
    if (lock.current || validationLock.current) return;
    const version = ++validationVersion.current;
    validationLock.current = true;
    setCheckingTsv(true);
    try {
      // A successful preview is never authority for writing: re-query immediately.
      const enriched = await validateTsvAssets(previewLines, version);
      if (!enriched || validationVersion.current !== version) return;
      setPreviewLines(enriched);
      if (enriched.some((row) => row.errors.length)) {
        setError("素材已变更，确认写入被阻止；请重新检查错误行。");
        return;
      }
      await applyPlans(enriched);
    } finally {
      validationLock.current = false;
      setCheckingTsv(false);
    }
  }

  const matrixDirtyCount = matrixAssignments().length;
  const matrixSlotTotal = readyShots.reduce((sum, entry) => sum + entry.slots.length, 0);
  const matrixFilledSlots = readyShots.reduce((sum, entry) => sum + rowFillStats(entry).filled, 0);
  const tsvValid = !checkingTsv && !!previewLines && previewLines.length > 0 && previewLines.every((row) => row.errors.length === 0);

  function onPanelKeyDown(event: { ctrlKey: boolean; metaKey: boolean; key: string; preventDefault(): void }) {
    if (!(event.ctrlKey || event.metaKey) || event.key !== "Enter" || busy || checkingTsv || loadingAssets) return;
    if (mode === "matrix" && matrixDirtyCount > 0) {
      event.preventDefault();
      void confirmMatrix();
    } else if (mode === "tsv" && tsvValid) {
      event.preventDefault();
      void confirmTsv();
    }
  }

  return (
    <section className="shot-batch-panel shot-batch-panel-open" aria-label="批量绑定外部素材" onKeyDown={onPanelKeyDown}>
      <div className="shot-batch-panel-heading">
        <div>
          <span className="section-label">项目生产</span>
          <h3>批量绑定外部素材</h3>
          <p className="shot-inline-note">把资源库中的外部图片/视频写入多个镜头的正式输入槽（首帧、尾帧、参考）。使用现有 OCC 保存，不创建任务，不启动生产队列。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onCancel} disabled={busy}>关闭</button>
      </div>
      <div className="shot-batch-stage-tabs" role="tablist" aria-label="绑定方式">
        <button type="button" role="tab" aria-selected={mode === "matrix"} className={mode === "matrix" ? "active" : ""} onClick={() => { validationVersion.current += 1; setPreviewLines(undefined); setMode("matrix"); }} disabled={busy || checkingTsv}>镜头矩阵</button>
        <button type="button" role="tab" aria-selected={mode === "tsv"} className={mode === "tsv" ? "active" : ""} onClick={() => { validationVersion.current += 1; setPreviewLines(undefined); setMode("tsv"); }} disabled={busy || checkingTsv}>表格文本</button>
      </div>
      {loadingAssets && <p className="shot-inline-note" role="status">正在加载外部导入素材…</p>}
      {!loadingAssets && !sourceAssets.length && <p className="empty-state" role="status">没有可用的外部导入素材。请先在资源库导入图片/视频/音频（来源：外部导入）。</p>}
      {mode === "matrix" ? (
        <>
          <div className="shot-batch-table-wrap">
            <table className="shot-batch-table" aria-label="多镜头正式槽绑定矩阵">
              <thead><tr>
                <th>镜头</th><th>配方</th><th>进度</th>
                <th>首帧 <small>({assetsForKey("first_frame").length})</small></th>
                <th>尾帧 <small>({assetsForKey("last_frame").length})</small></th>
                <th>参考图片 <small>({assetsForKey("reference_images").length})</small></th>
                <th>参考视频 <small>({assetsForKey("reference_videos").length})</small></th>
                <th>参考音频 <small>({assetsForKey("reference_audios").length})</small></th>
              </tr></thead>
              <tbody>
                {readyShots.map((entry) => {
                  const draft = matrix[entry.shot.id] ?? {};
                  const stats = rowFillStats(entry);
                  const cell = (key: BatchBindKey) => {
                    if (!entry.slots.includes(key)) return <td key={key}>—</td>;
                    const options = assetsForKey(key);
                    return (
                      <td key={key}>
                        <select aria-label={`镜头 ${entry.shot.ordinal + 1} ${BATCH_BIND_KEY_LABELS[key]}`} value={draft[key] ?? ""} disabled={busy || loadingAssets}
                          onChange={(event) => setMatrixValue(entry.shot.id, key, event.target.value)}>
                          <option value="">未绑定</option>
                          {options.map((asset) => <option key={asset.id} value={asset.id} title={asset.title}>外部 · {asset.title}</option>)}
                        </select>
                      </td>
                    );
                  };
                  return (
                    <tr key={entry.shot.id}>
                      <td><strong>{String(entry.shot.ordinal + 1).padStart(2, "0")} {entry.shot.name}</strong></td>
                      <td>{entry.recipe.name} · {entry.recipe.mode}</td>
                      <td>
                        <small aria-label={`镜头 ${entry.shot.ordinal + 1} 槽位进度`}>{stats.filled}/{stats.total}</small>
                        {" "}
                        <button type="button" className="quiet-button" disabled={busy || stats.filled === 0}
                          aria-label={`镜头 ${entry.shot.ordinal + 1} 复制到下方`}
                          title="把本行已选槽复制到下方匹配镜头"
                          onClick={() => copyRowDown(entry.shot.id)}>↓复制下方</button>
                      </td>
                      {cell("first_frame")}{cell("last_frame")}{cell("reference_images")}{cell("reference_videos")}{cell("reference_audios")}
                    </tr>
                  );
                })}
                {!readyShots.length && <tr><td colSpan={8}><p className="empty-state">没有已配置视频配方且含正式媒体槽的镜头。请先批量导入镜头并配置视频配方。</p></td></tr>}
              </tbody>
            </table>
          </div>
          <div className="shot-batch-selection-bar" aria-label="矩阵绑定操作">
            <span>草稿槽位：<strong>{matrixFilledSlots}/{matrixSlotTotal}</strong>（待写入 <strong>{matrixDirtyCount}</strong>）</span>
            <span>外部素材：<strong>{sourceAssets.length}</strong></span>
            <button type="button" className="quiet-button" onClick={() => fillColumn("first_frame")} disabled={busy || loadingAssets || !readyShots.length} title="用该列第一个已选值填充其余行">填充首帧列</button>
            <button type="button" className="quiet-button" onClick={() => fillColumn("last_frame")} disabled={busy || loadingAssets || !readyShots.length} title="用该列第一个已选值填充其余行">填充尾帧列</button>
            <button type="button" className="quiet-button" onClick={() => fillColumn("reference_images")} disabled={busy || loadingAssets || !readyShots.length}>填充参考图列</button>
            <button type="button" className="quiet-button" onClick={clearMatrix} disabled={busy || matrixFilledSlots === 0}>清空草稿</button>
            <button type="button" className="shot-primary-action" onClick={() => void confirmMatrix()} disabled={busy || loadingAssets || matrixDirtyCount === 0} title="Ctrl/⌘+Enter">
              {busy ? "正在写入…" : "写入正式输入"}
            </button>
          </div>
          <p className="shot-inline-note">提示：先在一格选好外部素材，再用「填充×列」铺开；Ctrl/⌘+Enter 等同写入。写入后仍不启动队列。</p>
        </>
      ) : (
        <>
          <label>
            <span className="section-label">绑定表</span>
            <textarea value={content} onChange={(event) => { validationVersion.current += 1; setContent(event.target.value); setPreviewLines(undefined); setError(undefined); setNotice(undefined); }}
              placeholder={"shot_ordinal\tinput_key\tasset_id\n1\tfirst_frame\tasset_xxx\n2\treference_images\tasset_yyy"}
              rows={8} aria-label="批量绑定表格内容" disabled={busy} />
          </label>
          <div className="shot-batch-selection-bar" aria-label="表格绑定操作">
            <button type="button" className="quiet-button" onClick={() => void checkTsv()} disabled={busy || checkingTsv}>{checkingTsv ? "正在校验素材…" : "检查"}</button>
            <button type="button" className="shot-primary-action" onClick={() => void confirmTsv()} disabled={busy || !tsvValid} title="Ctrl/⌘+Enter">{busy ? "正在写入…" : "确认写入"}</button>
          </div>
          {previewLines && (
            <div className="shot-batch-table-wrap">
              <table className="shot-batch-table" aria-label="批量绑定预览">
                <thead><tr><th>行</th><th>镜头</th><th>槽位</th><th>素材</th><th>校验</th></tr></thead>
                <tbody>
                  {previewLines.map((row) => (
                    <tr key={`${row.line}:${row.shotId}:${row.inputKey}:${row.assetId}`} className={row.errors.length ? "shot-batch-row-blocked" : ""}>
                      <td>{row.line}</td>
                      <td>{row.shotOrdinal ? `${String(row.shotOrdinal).padStart(2, "0")} ${row.shotName || ""}` : "—"}</td>
                      <td>{BATCH_BIND_KEY_LABELS[row.inputKey] ?? row.inputKey}</td>
                      <td>{row.assetId || "—"}</td>
                      <td>{row.errors.length ? <ul className="shot-batch-reasons">{row.errors.map((issue) => <li key={issue}>{issue}</li>)}</ul> : <span className="shot-batch-ready">可写入</span>}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
      {error && <p className="shot-recent-failure" role="alert">{error}</p>}
      {notice && <p className="shot-inline-note" role="status">{notice}</p>}
    </section>
  );
}
