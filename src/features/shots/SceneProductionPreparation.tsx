import { useEffect, useMemo, useState } from "react";
import { formatUiError } from "../../i18n/errorMessages";
import {
  admitSceneProduction,
  getSceneProductionPreflight,
  getShotProductionPlanDetail,
} from "../../services/tauriClient";
import type {
  ScenePreparationView,
  SceneProductionAdmissionResult,
  SceneProductionStage,
  ShotProductionPlanDetail,
  ShotProductionPlanSummary,
} from "../../types/productionPreparation";
import {
  MAX_PREPARATION_BATCH_ITEMS,
  preparationStatusLabel,
} from "../../types/productionPreparation";
import { ShotReadinessInspector } from "./ShotReadinessInspector";

export interface SceneProductionPreparationProps {
  projectId: string;
  sceneOptions: Array<{ value: string; label: string }>;
  currentSceneId?: string;
  initialStage?: SceneProductionStage;
  initialView?: ScenePreparationView;
  onOpenProductionQueue?: (batchId?: string) => void;
  onNotice?: (message: string) => void;
}

type PreparationBusyAction = "preflight" | "detail" | "admit";

interface PreparationError {
  code: string;
  message: string;
  technicalMessage?: string;
}

export function SceneProductionPreparation({
  projectId,
  sceneOptions,
  currentSceneId,
  initialStage = "image",
  initialView,
  onOpenProductionQueue,
  onNotice,
}: SceneProductionPreparationProps) {
  const firstSceneId = currentSceneId && sceneOptions.some((option) => option.value === currentSceneId)
    ? currentSceneId
    : sceneOptions[0]?.value ?? "";
  const [sceneId, setSceneId] = useState(firstSceneId);
  const [stage, setStage] = useState<SceneProductionStage>(initialStage);
  const [view, setView] = useState<ScenePreparationView | undefined>(
    initialView && initialView.projectId === projectId && initialView.sceneId === firstSceneId && initialView.stage === initialStage
      ? initialView
      : undefined,
  );
  const [selectedShotIds, setSelectedShotIds] = useState<Set<string>>(new Set());
  const [selectedShotId, setSelectedShotId] = useState<string>();
  const [detail, setDetail] = useState<ShotProductionPlanDetail>();
  const [busyAction, setBusyAction] = useState<PreparationBusyAction>();
  const [error, setError] = useState<PreparationError>();
  const [notice, setNotice] = useState<string>();
  const [admissionResult, setAdmissionResult] = useState<SceneProductionAdmissionResult>();

  const items = view?.items ?? [];
  const selectedCount = selectedShotIds.size;
  const readyItems = useMemo(
    () => items.filter((item) => item.status === "READY" && !item.alreadyPrepared),
    [items],
  );
  const isBusy = Boolean(busyAction);
  const selectedSummary = selectedShotId ? items.find((item) => item.shotId === selectedShotId) : undefined;

  useEffect(() => {
    if (currentSceneId && sceneOptions.some((option) => option.value === currentSceneId)) {
      setSceneId(currentSceneId);
    }
  }, [currentSceneId, sceneOptions]);

  useEffect(() => {
    if (!sceneId) {
      setView(undefined);
      setSelectedShotIds(new Set());
      setSelectedShotId(undefined);
      setDetail(undefined);
      return;
    }
    let active = true;
    setBusyAction("preflight");
    setError(undefined);
    setNotice(undefined);
    setAdmissionResult(undefined);
    setSelectedShotIds(new Set());
    setSelectedShotId(undefined);
    setDetail(undefined);
    void getSceneProductionPreflight({ projectId, sceneId, stage })
      .then((next) => {
        if (!active) return;
        setView(next);
      })
      .catch((value: unknown) => {
        if (active) {
          setView(undefined);
          setError(toPreparationError(value, "SCENE_PRODUCTION_PREFLIGHT_FAILED"));
        }
      })
      .finally(() => {
        if (active) setBusyAction(undefined);
      });
    return () => { active = false; };
  }, [projectId, sceneId, stage]);

  function clearFeedback() {
    setError(undefined);
    setNotice(undefined);
  }

  function selectScene(nextSceneId: string) {
    if (nextSceneId === sceneId) return;
    setSceneId(nextSceneId);
  }

  function selectStage(nextStage: SceneProductionStage) {
    if (nextStage === stage) return;
    setStage(nextStage);
  }

  function toggleShot(item: ShotProductionPlanSummary) {
    if (item.status !== "READY" || item.alreadyPrepared || isBusy) return;
    setSelectedShotIds((current) => {
      const next = new Set(current);
      if (next.has(item.shotId)) next.delete(item.shotId);
      else if (next.size < MAX_PREPARATION_BATCH_ITEMS) next.add(item.shotId);
      return next;
    });
  }

  function selectAllReady() {
    const limited = readyItems.slice(0, MAX_PREPARATION_BATCH_ITEMS);
    setSelectedShotIds(new Set(limited.map((item) => item.shotId)));
    if (readyItems.length > MAX_PREPARATION_BATCH_ITEMS) {
      setNotice("当前有 " + readyItems.length + " 个 READY 镜头，已只选择前 " + MAX_PREPARATION_BATCH_ITEMS + " 个。单批次最多 " + MAX_PREPARATION_BATCH_ITEMS + " 个镜头。");
    } else {
      setNotice("已选择 " + limited.length + " 个 READY 镜头。");
    }
  }

  async function openDetail(item: ShotProductionPlanSummary) {
    setSelectedShotId(item.shotId);
    setDetail(undefined);
    setError(undefined);
    setBusyAction("detail");
    try {
      setDetail(await getShotProductionPlanDetail({ projectId, shotId: item.shotId, stage }));
    } catch (value: unknown) {
      setError(toPreparationError(value, "SHOT_PRODUCTION_PLAN_DETAIL_FAILED"));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function admit() {
    if (!sceneId || !selectedCount || isBusy) return;
    const ids = [...selectedShotIds];
    if (!confirmAdmission(ids.length)) return;
    setBusyAction("admit");
    clearFeedback();
    try {
      const result = await admitSceneProduction({
        projectId,
        sceneId,
        stage,
        shotIds: ids,
        allowPartial: false,
      });
      setAdmissionResult(result);
      setSelectedShotIds(new Set());
      const message = "已加入生产队列：创建 " + result.createdCount + " 个，复用 " + result.alreadyPreparedCount + " 个，跳过 " + (result.skippedIncomplete + result.skippedBlocked) + " 个。";
      setNotice(message);
      onNotice?.(message);
      setView((current) => current ? markAdmittedItems(current, ids, result) : current);
    } catch (value: unknown) {
      setError(toPreparationError(value, "SCENE_PRODUCTION_ADMISSION_FAILED"));
    } finally {
      setBusyAction(undefined);
    }
  }

  if (!sceneOptions.length) {
    return <section className="scene-production-preparation" aria-label="场景生产准备"><div className="scene-production-empty"><strong>暂无可准备场景</strong><span>请先在现有内容结构中创建场景并分配镜头。</span></div></section>;
  }

  return (
    <section className="scene-production-preparation" aria-label="场景生产准备">
      <header className="scene-preparation-header">
        <div>
          <span className="section-label">PRODUCTION PREPARATION</span>
          <h2>场景生产准备</h2>
          <p>先解析当前场景的上下文与 ComfyUI 能力，再由你选择 READY 镜头加入现有生产队列。</p>
        </div>
        <span className="scene-preparation-safety">准备 ≠ 生成 · 加入 ≠ 启动</span>
      </header>

      <div className="scene-preparation-toolbar">
        <label>
          <span>场景</span>
          <select aria-label="准备场景选择" value={sceneId} onChange={(event) => selectScene(event.target.value)} disabled={isBusy}>
            {sceneOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </label>
        <div className="scene-preparation-stage-tabs" role="tablist" aria-label="准备阶段">
          {(["image", "video"] as SceneProductionStage[]).map((nextStage) => <button key={nextStage} type="button" role="tab" aria-selected={stage === nextStage} className={stage === nextStage ? "active" : ""} onClick={() => selectStage(nextStage)} disabled={isBusy}>{nextStage === "image" ? "图片" : "视频"}</button>)}
        </div>
        <span className="scene-preparation-evaluated">{busyAction === "preflight" ? "Live preflight 中…" : view?.evaluatedAt ? "检查于 " + formatTime(view.evaluatedAt) : "等待检查"}</span>
      </div>

      {error && <div className="scene-production-error" role="alert"><strong>{error.code}</strong><span>{error.message}</span>{error.technicalMessage && <details><summary>技术详情</summary><code>{error.technicalMessage}</code></details>}</div>}

      <div className="scene-preparation-summary" aria-label="场景准备统计">
        <SummaryCard label="总镜头" value={view?.total ?? 0} />
        <SummaryCard label="READY" value={view?.readyCount ?? 0} tone="ready" />
        <SummaryCard label="INCOMPLETE" value={view?.incompleteCount ?? 0} tone="incomplete" />
        <SummaryCard label="BLOCKED" value={view?.blockedCount ?? 0} tone="blocked" />
        <SummaryCard label="已准备" value={view?.preparedCount ?? 0} tone="prepared" />
      </div>

      <div className="scene-preparation-layout">
        <section className="scene-preparation-main" aria-label="镜头准备列表">
          <div className="scene-preparation-list-heading">
            <div><span className="section-label">SHOT PLAN</span><h3>{view?.sceneName || "当前场景"} · {stage === "image" ? "图片" : "视频"}</h3></div>
            <div className="scene-preparation-selection-actions">
              <span>{selectedCount}/{MAX_PREPARATION_BATCH_ITEMS} 已选</span>
              <button type="button" className="quiet-button" onClick={selectAllReady} disabled={isBusy || !readyItems.length}>选择全部 READY</button>
            </div>
          </div>
          {readyItems.length > MAX_PREPARATION_BATCH_ITEMS && <p className="scene-preparation-limit" role="status">READY 镜头超过 {MAX_PREPARATION_BATCH_ITEMS} 个；选择全部时只取前 {MAX_PREPARATION_BATCH_ITEMS} 个，单批次最多 {MAX_PREPARATION_BATCH_ITEMS} 个镜头。</p>}
          {!view && busyAction === "preflight" && <div className="scene-preparation-empty"><strong>正在检查场景</strong><span>只执行一次场景级 Live preflight，不会创建 Batch、Task 或生成任务。</span></div>}
          {view && !items.length && <div className="scene-preparation-empty"><strong>场景暂无镜头</strong><span>请回到现有内容结构分配镜头。</span></div>}
          {items.length > 0 && <div className="scene-preparation-shot-list">{items.map((item) => <ShotPreparationCard key={item.shotId} item={item} selected={selectedShotIds.has(item.shotId)} onToggle={() => toggleShot(item)} onInspect={() => void openDetail(item)} disabled={isBusy} />)}</div>}
          <div className="scene-preparation-admission-bar">
            <div><strong>{selectedCount ? "将加入 " + selectedCount + " 个 READY 镜头" : "请选择 READY 镜头"}</strong><span>后端会再次 Live preflight；页面缓存不会直接授权生产。</span></div>
            <button type="button" onClick={() => void admit()} disabled={isBusy || selectedCount === 0}>{busyAction === "admit" ? "加入中…" : "加入生产"}</button>
          </div>
        </section>

        <ShotReadinessInspector detail={detail} loading={busyAction === "detail"} error={selectedShotId && !detail ? error?.message : undefined} onRetry={selectedSummary ? () => void openDetail(selectedSummary) : undefined} />
      </div>

      {notice && <div className="scene-production-notice" role="status">{notice}</div>}
      {admissionResult && <section className="scene-preparation-success" aria-label="加入生产结果">
        <div><strong>已加入生产队列</strong><span>已创建 Batch / BatchItem；没有启动队列，也没有提交生成。</span></div>
        <dl><div><dt>创建</dt><dd>{admissionResult.createdCount}</dd></div><div><dt>复用</dt><dd>{admissionResult.alreadyPreparedCount}</dd></div><div><dt>跳过</dt><dd>{admissionResult.skippedIncomplete + admissionResult.skippedBlocked}</dd></div></dl>
        <button type="button" className="quiet-button" onClick={() => onOpenProductionQueue?.(admissionResult.batchId ?? admissionResult.createdBatchIds?.[0])} disabled={!onOpenProductionQueue}>前往生产队列</button>
      </section>}
      <p className="scene-preparation-footer-note">生产队列仍由现有 Runbook 手动操作；本页面只负责准备与加入生产。</p>
    </section>
  );
}

function SummaryCard({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return <div className={"scene-preparation-summary-card" + (tone ? " scene-preparation-summary-" + tone : "")}><span>{label}</span><strong>{value}</strong></div>;
}

function ShotPreparationCard({ item, selected, onToggle, onInspect, disabled }: { item: ShotProductionPlanSummary; selected: boolean; onToggle: () => void; onInspect: () => void; disabled: boolean }) {
  const isSelectable = item.status === "READY" && !item.alreadyPrepared;
  const stale = (item.stalePreparedBatchIds?.length ?? 0) > 0;
  return (
    <article className={"scene-preparation-shot-card scene-preparation-shot-" + item.status.toLowerCase()} data-shot-id={item.shotId}>
      <label className="scene-preparation-shot-select">
        <input type="checkbox" checked={selected} onChange={onToggle} disabled={disabled || !isSelectable} aria-label={"选择 " + item.name} />
        <span className="scene-preparation-shot-check" aria-hidden="true" />
      </label>
      <div className="scene-preparation-shot-thumb" aria-label={item.thumbnailUrl ? item.name + " 缩略图" : "暂无缩略图"}>
        {item.thumbnailUrl ? <img src={item.thumbnailUrl} alt="" /> : <span>SHOT</span>}
      </div>
      <div className="scene-preparation-shot-copy">
        <div className="scene-preparation-shot-heading"><strong>{item.name}</strong><StatusBadge status={item.status} /></div>
        <small>{item.shotId} · #{item.ordinal + 1}{item.legacy ? " · Legacy Shot" : ""}</small>
        <div className="scene-preparation-shot-meta">
          <span>{item.characterNames?.length ? "角色：" + item.characterNames.slice(0, 2).join("、") : (item.characterCount ?? 0) + " 个角色"}</span>
          <span>{item.sceneProfileName ?? "无场景 Profile"}</span>
          <span>{item.referenceCount ?? 0} 个参考</span>
          <span>评分 {item.score}</span>
        </div>
        <div className="scene-preparation-shot-foot">
          <span>{item.currentStageStatus || "未开始"}</span>
          {item.alreadyPrepared && <em>已准备</em>}
          {stale && <em className="scene-preparation-stale">已有旧上下文准备版本</em>}
          {item.warningCount > 0 && <em className="scene-preparation-warning">{item.warningCount} 个警告</em>}
        </div>
        {(item.blockers?.length ?? 0) > 0 && <p className="scene-preparation-card-reason">{item.blockers?.[0]}</p>}
      </div>
      <button type="button" className="quiet-button scene-preparation-inspect" onClick={onInspect} disabled={disabled}>查看详情</button>
    </article>
  );
}

function StatusBadge({ status }: { status: string }) {
  return <span className={"scene-preparation-status scene-preparation-status-" + status.toLowerCase()}>{preparationStatusLabel(status)}</span>;
}

function markAdmittedItems(view: ScenePreparationView, shotIds: string[], result: SceneProductionAdmissionResult): ScenePreparationView {
  const ids = new Set(shotIds);
  const batchId = result.batchId ?? result.createdBatchIds?.[0];
  return {
    ...view,
    preparedCount: view.preparedCount + result.createdCount,
    items: view.items.map((item) => ids.has(item.shotId)
      ? { ...item, alreadyPrepared: true, existingBatchIds: unique([...(item.existingBatchIds ?? []), ...(batchId ? [batchId] : [])]) }
      : item),
  };
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

function confirmAdmission(count: number): boolean {
  if (typeof window === "undefined" || typeof window.confirm !== "function") return true;
  return window.confirm("将加入 " + count + " 个 READY 镜头。后端会重新检查上下文；加入生产不会启动队列。是否继续？");
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}

function toPreparationError(value: unknown, fallbackCode: string): PreparationError {
  const formatted = formatUiError(value);
  return { code: formatted.code ?? fallbackCode, message: formatted.message, technicalMessage: formatted.technicalMessage };
}

export function preparationSelectionLimit(readyCount: number): number {
  return Math.min(readyCount, MAX_PREPARATION_BATCH_ITEMS);
}
