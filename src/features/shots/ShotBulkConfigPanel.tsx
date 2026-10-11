import { useEffect, useMemo, useRef, useState } from "react";
import {
  admitProjectProduction,
  getProjectProductionPreflight,
} from "../../services/tauriClient";
import type { ShotStage, ShotView } from "../../types/shot";
import type {
  ProjectPreparationView,
  ProjectProductionAdmissionResult,
} from "../../types/productionPreparation";
import { preparationStatusLabel } from "../../types/productionPreparation";
import { toUserMessage } from "../../i18n/errorMessages";
import { deriveStageStatus, statusLabel } from "./shotDomain";
import "./ProjectProductionPipeline.css";

export type BulkSelectionPreset = "all" | "ready" | "unconfigured";
export type BulkPreparationFilter = "all" | "ready" | "prepared" | "blocked" | "done";

export interface ShotBulkConfigPanelProps {
  projectId: string;
  shots: ShotView[];
  onRefresh?: () => Promise<void>;
  onNotice?: (message: string) => void;
  onError?: (message?: string) => void;
  onConfigureStage?: (stage: ShotStage, shotIds: string[]) => void | Promise<void>;
  onBulkPrompt?: (stage: ShotStage, shotIds: string[], promptText: string) => void | Promise<void>;
  onOpenProductionQueue?: (batchId?: string) => void | Promise<void>;
  onOpenShot?: (shotId: string) => void;
}

export const MAX_BULK_PREPARATION_ITEMS = 100;

// Only fixed, locally authored validation messages may bypass IPC error
// sanitization. Unknown backend/transport errors keep the shared formatter.
class PreparationUiError extends Error {}
const preparationErrorMessage = (error: unknown) => error instanceof PreparationUiError ? error.message : toUserMessage(error);

export function shotHasStageConfig(shot: ShotView, stage: ShotStage): boolean {
  return shot.stageConfigs.some((config) => config.stage === stage);
}

export function bulkSelectionIds(
  shots: ShotView[],
  preset: BulkSelectionPreset,
  stage: ShotStage,
): string[] {
  return shots
    .filter((shot) => {
      if (preset === "all") return true;
      if (preset === "ready") return deriveStageStatus(shot, stage) === "READY";
      return !shotHasStageConfig(shot, stage);
    })
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id))
    .map((shot) => shot.id);
}

function stageLabel(stage: ShotStage): string {
  return stage === "image" ? "图片阶段" : "视频阶段";
}

function promptPreview(text: string): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (normalized.length <= 96) return normalized || "未配置提示词";
  return `${normalized.slice(0, 93)}…`;
}

export function ShotBulkConfigPanel(props: ShotBulkConfigPanelProps) {
  // Project identity is a lifecycle boundary, not merely a filter on shared IDs.
  return <ScopedShotBulkConfigPanel key={props.projectId} {...props} />;
}

function ScopedShotBulkConfigPanel({
  projectId,
  shots,
  onRefresh,
  onNotice,
  onError,
  onConfigureStage,
  onBulkPrompt,
  onOpenProductionQueue,
  onOpenShot,
}: ShotBulkConfigPanelProps) {
  const [stage, setStage] = useState<ShotStage>("video");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [busyAction, setBusyAction] = useState<string>();
  const [localError, setLocalError] = useState<string>();
  const [localNotice, setLocalNotice] = useState<string>();
  const [promptText, setPromptText] = useState("");
  const [projectPlan, setProjectPlan] = useState<ProjectPreparationView>();
  const [planLoading, setPlanLoading] = useState(false);
  const [allowPartial, setAllowPartial] = useState(false);
  const [preparationResult, setPreparationResult] = useState<ProjectProductionAdmissionResult>();
  const [filter, setFilter] = useState<BulkPreparationFilter>("all");
  const shotIdKey = shots.map((shot) => shot.id).join("|");
  const mounted = useRef(true);
  const actionLock = useRef(false);
  const planRequest = useRef(0);
  const owner = JSON.stringify([projectId, stage]);
  const currentOwner = useRef(owner);
  currentOwner.current = owner;
  const isCurrent = (requestedOwner: string) => mounted.current && currentOwner.current === requestedOwner;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  useEffect(() => {
    const available = new Set(shots.map((shot) => shot.id));
    setSelectedIds((current) => new Set([...current].filter((id) => available.has(id))));
  }, [shotIdKey]);

  const selectedCount = selectedIds.size;
  const selectedOverLimit = selectedCount > MAX_BULK_PREPARATION_ITEMS;
  const planByShotId = useMemo(
    () => new Map((projectPlan?.items ?? []).map((item) => [item.shotId, item])),
    [projectPlan],
  );
  const visibleShots = useMemo(
    () => shots.filter((shot) => {
      if (shot.projectId !== projectId) return false;
      const item = planByShotId.get(shot.id);
      switch (filter) {
        case "ready": return item?.status === "READY" && !item.alreadyPrepared;
        case "prepared": return Boolean(item?.alreadyPrepared);
        case "blocked": return item?.status === "BLOCKED";
        case "done": return stage === "video" ? Boolean(shot.selectedVideoAssetId) : Boolean(shot.selectedImageAssetId);
        case "all": return true;
      }
    }).sort((a, b) => a.ordinal - b.ordinal || a.id.localeCompare(b.id)),
    [filter, planByShotId, shots, stage, projectId],
  );
  const selectedIdsInOrder = useMemo(
    () => shots
      .filter((shot) => selectedIds.has(shot.id))
      .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id))
      .map((shot) => shot.id),
    [selectedIds, shots],
  );

  function reportError(message?: string) {
    setLocalNotice(undefined);
    setLocalError(message);
    onError?.(message);
  }

  function reportNotice(message: string) {
    setLocalError(undefined);
    setLocalNotice(message);
    onNotice?.(message);
  }

  async function refreshPlan() {
    const requestedOwner = owner;
    const requestId = ++planRequest.current;
    setPlanLoading(true);
    try {
      const next = await getProjectProductionPreflight({ projectId, stage });
      if (!isCurrent(requestedOwner) || requestId !== planRequest.current) return;
      const ownedIds = new Set(shots.filter(shot => shot.projectId === projectId).map(shot => shot.id));
      if (next.projectId !== projectId || next.stage !== stage || next.items.some(item => !ownedIds.has(item.shotId))) {
        throw new PreparationUiError("生产预检归属不一致，请刷新项目后重试。");
      }
      setProjectPlan(next);
      return next;
    } finally {
      if (isCurrent(requestedOwner) && requestId === planRequest.current) setPlanLoading(false);
    }
  }

  useEffect(() => {
    setProjectPlan(undefined);
    setPreparationResult(undefined);
    setSelectedIds(new Set());
    setLocalError(undefined);
    setLocalNotice(undefined);
    void refreshPlan().catch((error: unknown) => { if (isCurrent(owner)) reportError(preparationErrorMessage(error)); });
    return () => { planRequest.current++; };
    // A saved Shot edit invalidates readiness, even when its ID is unchanged.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, stage, shots]);

  function applyPreset(preset: BulkSelectionPreset) {
    const ids = preset === "ready" && projectPlan
      ? projectPlan.items
        .filter((item) => item.status === "READY" && !item.alreadyPrepared)
        .sort((left, right) => left.ordinal - right.ordinal || left.shotId.localeCompare(right.shotId))
        .slice(0, MAX_BULK_PREPARATION_ITEMS)
        .map((item) => item.shotId)
      : preset === "ready" ? [] : bulkSelectionIds(shots.filter(shot => shot.projectId === projectId), preset, stage);
    setSelectedIds(new Set(ids));
    setPreparationResult(undefined);
    reportNotice(
      preset === "all"
        ? `已选择全部 ${shots.length} 个镜头。`
        : preset === "ready"
          ? `已选择当前${stageLabel(stage)}待启动的镜头。`
          : `已选择当前${stageLabel(stage)}未配置的镜头。`,
    );
  }

  function toggleShot(shotId: string) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(shotId)) next.delete(shotId);
      else next.add(shotId);
      return next;
    });
    setLocalError(undefined);
    setLocalNotice(undefined);
  }

  async function runAction(
    actionName: string,
    action: (shotIds: string[]) => void | Promise<void>,
    successMessage: string,
    refreshWorkspace = true,
  ) {
    if (actionLock.current) return;
    if (!selectedIdsInOrder.length) {
      reportError("请先选择至少一个镜头。");
      return;
    }
    const requestedOwner = owner;
    actionLock.current = true;
    setBusyAction(actionName);
    reportError(undefined);
    try {
      await action(selectedIdsInOrder);
      if (!isCurrent(requestedOwner)) return;
      reportNotice(successMessage);
      if (refreshWorkspace) await onRefresh?.();
    } catch (error: unknown) {
      if (isCurrent(requestedOwner)) reportError(preparationErrorMessage(error));
    } finally {
      if (isCurrent(requestedOwner)) { actionLock.current = false; setBusyAction(undefined); }
    }
  }

  async function configureStage(nextStage: ShotStage) {
    if (!onConfigureStage) {
      reportNotice(`已保留${stageLabel(nextStage)}批量配置入口；等待上层接入现有阶段配置命令。`);
      return;
    }
    await runAction(
      `configure-${nextStage}`,
      (shotIds) => onConfigureStage(nextStage, shotIds),
      `已提交 ${selectedCount} 个镜头的${stageLabel(nextStage)}批量配置。`,
    );
  }

  async function assignPrompt() {
    if (!onBulkPrompt) {
      reportNotice("已保留批量提示词入口；当前组件不会绕过镜头服务直接写数据库。 ");
      return;
    }
    if (!promptText.trim()) {
      reportError("请输入要批量应用的提示词。清除来源标记请使用后端批量命令。" );
      return;
    }
    await runAction(
      "prompt",
      (shotIds) => onBulkPrompt(stage, shotIds, promptText),
      `已提交 ${selectedCount} 个镜头的批量提示词操作；保存后仍按快照语义生效。`,
    );
  }

  async function prepareBatch(nextStage: ShotStage) {
    if (nextStage !== "video" || stage !== "video") return;
    const requestedOwner = owner;
    setPreparationResult(undefined);
    await runAction(
      `prepare-${nextStage}`,
      async (shotIds) => {
        if (selectedOverLimit) {
          throw new PreparationUiError(`本次最多准备 ${MAX_BULK_PREPARATION_ITEMS} 个镜头。`);
        }
        const fresh = await refreshPlan();
        if (!isCurrent(requestedOwner)) return;
        if (!fresh) throw new PreparationUiError("镜头资料已变更，预检已失效，请重新准备。");
        const selected = fresh.items.filter(item => shotIds.includes(item.shotId));
        if (selected.length !== shotIds.length) throw new PreparationUiError("所选镜头已变更或不属于当前项目。");
        if (!allowPartial && selected.some(item => item.status !== "READY" || item.alreadyPrepared)) {
          throw new PreparationUiError("最新预检未通过或镜头已经准备，请检查阻塞原因和陈旧快照。");
        }
        const result = await admitProjectProduction({
          projectId,
          stage: nextStage,
          shotIds,
          allowPartial,
        });
        if (!isCurrent(requestedOwner)) return;
        if (result.projectId !== projectId || result.stage !== nextStage) throw new PreparationUiError("批次准入回执归属不一致。");
        setPreparationResult(result);
        setSelectedIds(new Set());
        await refreshPlan();
        if (!isCurrent(requestedOwner)) return;
        reportNotice(`${stageLabel(nextStage)}已准备 ${result.createdCount} 个镜头；不会启动 GPU，请前往生产队列明确启动。`);
      },
      `${stageLabel(nextStage)}准备操作已完成；批次仍为待启动状态。`,
      // Admission changes batches, not Shot drafts. Fresh plan data above is
      // authoritative; a whole-workspace reload would erase this receipt.
      false,
    );
  }

  return (
    <section className="shot-bulk-config-panel" aria-label="批量镜头配置">
      <div className="pipeline-section-heading">
        <div>
          <span className="section-label">批量配置</span>
          <h3>镜头批量生产入口</h3>
          <p className="pipeline-muted">按镜头顺序准备视频；T2V 不要求图片，其他模式使用各自正式输入。未归档镜头同样可准备。</p>
        </div>
        <span className="pipeline-selection-count">{selectedCount} 个镜头已选择</span>
      </div>

      <div className="pipeline-stage-tabs" role="tablist" aria-label="批量配置阶段">
        {(["image", "video"] as const).map((nextStage) => (
          <button
            key={nextStage}
            type="button"
            className={stage === nextStage ? "active" : ""}
            role="tab"
            aria-selected={stage === nextStage}
            onClick={() => setStage(nextStage)}
            disabled={Boolean(busyAction)}
          >
            {nextStage === "image" ? "图片历史" : "视频阶段"}
          </button>
        ))}
      </div>

      <div className="pipeline-selection-toolbar" aria-label="镜头选择操作">
        <button type="button" className="quiet-button" onClick={() => applyPreset("all")} disabled={Boolean(busyAction)}>全选</button>
        <button type="button" className="quiet-button" onClick={() => setSelectedIds(new Set())} disabled={Boolean(busyAction) || !selectedCount}>取消全选</button>
        <button type="button" className="quiet-button" onClick={() => applyPreset("ready")} disabled={Boolean(busyAction)}>选择就绪项</button>
        <button type="button" className="quiet-button" onClick={() => applyPreset("unconfigured")} disabled={Boolean(busyAction)}>选择未配置</button>
        <span>{selectedCount} / {shots.length} 已选择</span>
      </div>

      <div className="pipeline-filter-tabs" role="group" aria-label="项目生产计划筛选">
        {(["all", "ready", "prepared", "blocked", "done"] as const).map((value) => (
          <button key={value} type="button" className={filter === value ? "active" : ""} onClick={() => setFilter(value)} disabled={Boolean(busyAction)}>
            {{ all: "全部", ready: "可准备", prepared: "已准备", blocked: "有阻塞", done: "已完成" }[value]}
          </button>
        ))}
      </div>

      <div className="pipeline-action-grid">
        <button type="button" onClick={() => void configureStage("image")} disabled>
          配置图片阶段
        </button>
        <button type="button" onClick={() => void configureStage("video")} disabled={Boolean(busyAction) || !selectedCount}>
          配置视频阶段
        </button>
        <button type="button" onClick={() => void assignPrompt()} disabled={Boolean(busyAction) || !selectedCount}>
          批量应用提示词
        </button>
        <button type="button" disabled>
          {busyAction === "prepare-image" ? "正在准备图片生产…" : "准备图片生产"}
        </button>
        <button type="button" onClick={() => void prepareBatch("video")} disabled={stage !== "video" || Boolean(busyAction) || planLoading || !selectedCount || selectedOverLimit}>
          {busyAction === "prepare-video" ? "正在准备视频生产…" : "准备视频生产"}
        </button>
      </div>
      {stage === "image" && <p className="pipeline-muted">图片生成已退休；仅保留历史记录、候选和提示词编辑，不创建新图片生产。</p>}
      <button type="button" className="quiet-button" onClick={() => void refreshPlan().catch(error => { if (isCurrent(owner)) reportError(toUserMessage(error)); })} disabled={Boolean(busyAction) || planLoading}>重新检查资格</button>

      <div className="pipeline-preparation-summary" aria-label="项目生产准备计划">
        <span>{planLoading ? "正在进行项目级 Live preflight…" : `当前${stageLabel(stage)}计划：${projectPlan?.total ?? 0} 个镜头`}</span>
        <span>可准备 {projectPlan?.readyCount ?? 0} · 已准备 {projectPlan?.preparedCount ?? 0} · 阻塞 {projectPlan?.blockedCount ?? 0}</span>
        <label><input type="checkbox" checked={allowPartial} onChange={(event) => setAllowPartial(event.target.checked)} disabled={Boolean(busyAction) || planLoading} /> 允许部分准备：只准备当前可准备镜头，阻塞镜头不写入本批次</label>
        <small>可准备只是准备前状态；已创建批次为待启动，不等于运行中。只有生产队列的“开始生产”会执行。</small>
      </div>
      {selectedOverLimit && <p className="pipeline-preparation-limit" role="alert">本次最多准备 {MAX_BULK_PREPARATION_ITEMS} 个镜头；系统不会自动拆分批次，请减少选择或使用“选择就绪项”。</p>}

      <label className="pipeline-prompt-editor">
        <span>批量应用当前阶段提示词</span>
        <textarea value={promptText} onChange={(event) => setPromptText(event.target.value)} rows={3} placeholder="输入后点击“批量应用提示词”；文本会以当前阶段快照写入所选镜头。" disabled={Boolean(busyAction)} />
      </label>

      <div className="pipeline-shot-table-wrap">
        <table className="pipeline-shot-table">
          <thead>
            <tr><th aria-label="选择" /><th>镜头</th><th>当前提示词快照</th><th>当前阶段计划</th><th>工作流 / Recipe</th><th>参考</th><th>图片状态</th><th>视频状态</th></tr>
          </thead>
          <tbody>
            {visibleShots.map((shot) => (
              <tr key={shot.id} className={selectedIds.has(shot.id) ? "pipeline-shot-row-selected" : ""}>
                <td>
                  <input
                    type="checkbox"
                    checked={selectedIds.has(shot.id)}
                    onChange={() => toggleShot(shot.id)}
                    disabled={Boolean(busyAction)}
                    aria-label={`选择 ${shot.name}`}
                  />
                </td>
                <td><strong>{String(shot.ordinal + 1).padStart(2, "0")} · {shot.name}</strong><small>{shot.id}</small>{onOpenShot && <button type="button" className="quiet-button" onClick={() => onOpenShot(shot.id)}>编辑输入与查看结果</button>}</td>
                <td><span className="pipeline-prompt-preview">{promptPreview(shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptText ?? shot.promptText)}</span><small>{shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptEntryId && shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptVersionId ? `提示词库版本 ${shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptVersionId?.slice(-8)}` : "手工/导入阶段快照"}</small></td>
                <td><strong>{planByShotId.get(shot.id)?.status ? preparationStatusLabel(planByShotId.get(shot.id)!.status) : "等待正式预检"}</strong><small>{planByShotId.get(shot.id)?.alreadyPrepared ? "已准备，待启动" : planByShotId.get(shot.id)?.blockers?.join("；") ?? "可继续检查"}</small>{Boolean(planByShotId.get(shot.id)?.stalePreparedBatchIds?.length) && <small>陈旧准备版本；旧批次快照保留</small>}</td>
                <td><small>{planByShotId.get(shot.id)?.workflowVersionId && planByShotId.get(shot.id)?.recipeId ? `${planByShotId.get(shot.id)?.workflowVersionId} / ${planByShotId.get(shot.id)?.recipeId}` : "未配置精确配对"}</small></td>
                <td>{planByShotId.get(shot.id)?.referenceCount ?? 0}</td>
                <td><span className="pipeline-status-chip">{statusLabel(deriveStageStatus(shot, "image"))}</span></td>
                <td><span className="pipeline-status-chip">{statusLabel(deriveStageStatus(shot, "video"))}</span></td>
              </tr>
            ))}
            {!visibleShots.length && <tr><td colSpan={8}><p className="empty-state">当前筛选没有匹配的镜头。</p></td></tr>}
          </tbody>
        </table>
      </div>

      <p className="pipeline-human-review-note"><strong>人工审核保持不变：</strong>生成成功、候选选用和人工审核通过是三个独立状态；准备与生成不会自动选择候选或通过审核。</p>
      {preparationResult && <div className="pipeline-preparation-result" role="status"><strong>已准备完成 · {preparationResult.createdCount} 个镜头</strong><span>已创建待启动批次；当前尚未开始生产。已准备 {preparationResult.alreadyPreparedCount} · 阻塞跳过 {preparationResult.skippedBlocked} · 不完整跳过 {preparationResult.skippedIncomplete}。</span>{onOpenProductionQueue && preparationResult.createdCount > 0 && <button type="button" className="quiet-button" onClick={() => void onOpenProductionQueue(preparationResult.batchId ?? preparationResult.createdBatchIds?.[0])}>打开生产队列</button>}</div>}
      {localNotice && <p className="studio-notice">{localNotice}</p>}
      {localError && <p className="error-message" role="alert">{localError}</p>}
    </section>
  );
}
