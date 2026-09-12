import { useEffect, useMemo, useState } from "react";
import {
  admitProjectProduction,
  getProjectProductionPreflight,
} from "../../services/tauriClient";
import type { ShotStage, ShotView } from "../../types/shot";
import type {
  ProjectPreparationView,
  ProjectProductionAdmissionResult,
} from "../../types/productionPreparation";
import { toUserMessage } from "../../i18n/errorMessages";
import { deriveShotStatus, deriveStageStatus, statusLabel } from "./shotDomain";
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
  onOpenProductionQueue?: () => void | Promise<void>;
}

export const MAX_BULK_PREPARATION_ITEMS = 100;

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

export function ShotBulkConfigPanel({
  projectId,
  shots,
  onRefresh,
  onNotice,
  onError,
  onConfigureStage,
  onBulkPrompt,
  onOpenProductionQueue,
}: ShotBulkConfigPanelProps) {
  const [stage, setStage] = useState<ShotStage>("image");
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
      const item = planByShotId.get(shot.id);
      switch (filter) {
        case "ready": return item?.status === "READY" && !item.alreadyPrepared;
        case "prepared": return Boolean(item?.alreadyPrepared);
        case "blocked": return item?.status === "BLOCKED";
        case "done": return deriveShotStatus(shot) === "COMPLETED";
        case "all": return true;
      }
    }),
    [filter, planByShotId, shots],
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

  useEffect(() => {
    let active = true;
    setPlanLoading(true);
    setProjectPlan(undefined);
    setPreparationResult(undefined);
    void getProjectProductionPreflight({ projectId, stage })
      .then((next) => {
        if (active) setProjectPlan(next);
      })
      .catch((error: unknown) => {
        if (active) reportError(toUserMessage(error));
      })
      .finally(() => {
        if (active) setPlanLoading(false);
      });
    return () => { active = false; };
  }, [projectId, stage]);

  function applyPreset(preset: BulkSelectionPreset) {
    const ids = preset === "ready" && projectPlan
      ? projectPlan.items
        .filter((item) => item.status === "READY" && !item.alreadyPrepared)
        .sort((left, right) => left.ordinal - right.ordinal || left.shotId.localeCompare(right.shotId))
        .slice(0, MAX_BULK_PREPARATION_ITEMS)
        .map((item) => item.shotId)
      : bulkSelectionIds(shots, preset, stage);
    setSelectedIds(new Set(ids));
    setPreparationResult(undefined);
    reportNotice(
      preset === "all"
        ? `已选择全部 ${shots.length} 个镜头。`
        : preset === "ready"
          ? `已选择当前${stageLabel(stage)}待生成的镜头。`
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
  ) {
    if (!selectedIdsInOrder.length) {
      reportError("请先选择至少一个镜头。");
      return;
    }
    setBusyAction(actionName);
    reportError(undefined);
    try {
      await action(selectedIdsInOrder);
      reportNotice(successMessage);
      await onRefresh?.();
    } catch (error: unknown) {
      reportError(toUserMessage(error));
    } finally {
      setBusyAction(undefined);
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
    await runAction(
      `prepare-${nextStage}`,
      async (shotIds) => {
        if (selectedOverLimit) {
          throw new Error(`本次最多准备 ${MAX_BULK_PREPARATION_ITEMS} 个镜头。`);
        }
        const result = await admitProjectProduction({
          projectId,
          stage: nextStage,
          shotIds,
          allowPartial,
        });
        setPreparationResult(result);
        setSelectedIds(new Set());
        void getProjectProductionPreflight({ projectId, stage: nextStage })
          .then((nextPlan) => {
            if (nextStage === stage) setProjectPlan(nextPlan);
          })
          .catch(() => undefined);
        reportNotice(`${stageLabel(nextStage)}已准备 ${result.createdCount} 个镜头；不会启动 GPU，请前往生产队列明确启动。`);
      },
      `${stageLabel(nextStage)}准备操作已完成；批次仍为待启动状态。`,
    );
  }

  return (
    <section className="shot-bulk-config-panel" aria-label="批量镜头配置">
      <div className="pipeline-section-heading">
        <div>
          <span className="section-label">批量配置</span>
          <h3>镜头批量生产入口</h3>
          <p className="pipeline-muted">先选择镜头，再进入图片/视频配置、提示词或现有镜头批次队列。</p>
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
          >
            {nextStage === "image" ? "图片阶段" : "视频阶段"}
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
        <button type="button" onClick={() => void configureStage("image")} disabled={Boolean(busyAction) || !selectedCount}>
          配置图片阶段
        </button>
        <button type="button" onClick={() => void configureStage("video")} disabled={Boolean(busyAction) || !selectedCount}>
          配置视频阶段
        </button>
        <button type="button" onClick={() => void assignPrompt()} disabled={Boolean(busyAction) || !selectedCount}>
          批量应用提示词
        </button>
        <button type="button" onClick={() => void prepareBatch("image")} disabled={Boolean(busyAction) || planLoading || !selectedCount || selectedOverLimit}>
          {busyAction === "prepare-image" ? "正在准备图片生产…" : "准备图片生产"}
        </button>
        <button type="button" onClick={() => void prepareBatch("video")} disabled={Boolean(busyAction) || planLoading || !selectedCount || selectedOverLimit}>
          {busyAction === "prepare-video" ? "正在准备视频生产…" : "准备视频生产"}
        </button>
      </div>

      <div className="pipeline-preparation-summary" aria-label="项目生产准备计划">
        <span>{planLoading ? "正在进行项目级 Live preflight…" : `当前${stageLabel(stage)}计划：${projectPlan?.total ?? 0} 个镜头`}</span>
        <span>READY {projectPlan?.readyCount ?? 0} · 已准备 {projectPlan?.preparedCount ?? 0} · 阻塞 {projectPlan?.blockedCount ?? 0}</span>
        <label><input type="checkbox" checked={allowPartial} onChange={(event) => setAllowPartial(event.target.checked)} disabled={Boolean(busyAction) || planLoading} /> 允许部分准备：只准备当前 READY 镜头，阻塞镜头不写入本批次</label>
        <small>READY 只是可准备；已准备不等于 Running。启动仍只在生产队列中由用户明确执行。</small>
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
                <td><strong>{String(shot.ordinal + 1).padStart(2, "0")} · {shot.name}</strong><small>{shot.id}</small></td>
                <td><span className="pipeline-prompt-preview">{promptPreview(shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptText ?? shot.promptText)}</span><small>{shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptEntryId && shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptVersionId ? `提示词库版本 ${shot.stagePrompts?.find((prompt) => prompt.stage === stage)?.promptVersionId?.slice(-8)}` : "手工/导入阶段快照"}</small></td>
                <td><strong>{planByShotId.get(shot.id)?.status ?? statusLabel(deriveStageStatus(shot, stage))}</strong><small>{planByShotId.get(shot.id)?.alreadyPrepared ? "已准备" : planByShotId.get(shot.id)?.blockers?.[0] ?? "可继续检查"}</small></td>
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

      <p className="pipeline-human-review-note"><strong>人工审核保持不变：</strong>批量操作不会自动选择第一张图片、最新图片或第一个视频；图片生成后停在图片审核，视频生成后停在视频审核。</p>
      {preparationResult && <div className="pipeline-preparation-result" role="status"><strong>已准备 {preparationResult.createdCount} 个镜头</strong><span>已准备 {preparationResult.alreadyPreparedCount} · 阻塞跳过 {preparationResult.skippedBlocked} · 不完整跳过 {preparationResult.skippedIncomplete}。没有启动队列或创建 Task。</span>{onOpenProductionQueue && <button type="button" className="quiet-button" onClick={() => void onOpenProductionQueue()}>打开生产队列</button>}</div>}
      {localNotice && <p className="studio-notice">{localNotice}</p>}
      {localError && <p className="error-message" role="alert">{localError}</p>}
    </section>
  );
}
