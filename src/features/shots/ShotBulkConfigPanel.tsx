import { useEffect, useMemo, useState } from "react";
import {
  createShotBatch,
  planShotBatch,
  startProductionQueue,
} from "../../services/tauriClient";
import type { ShotStage, ShotView } from "../../types/shot";
import { toUserMessage } from "../../i18n/errorMessages";
import { deriveStageStatus, statusLabel } from "./shotDomain";
import "./ProjectProductionPipeline.css";

export type BulkSelectionPreset = "all" | "ready" | "unconfigured";

export interface ShotBulkConfigPanelProps {
  projectId: string;
  shots: ShotView[];
  onRefresh?: () => Promise<void>;
  onNotice?: (message: string) => void;
  onError?: (message?: string) => void;
  onConfigureStage?: (stage: ShotStage, shotIds: string[]) => void | Promise<void>;
  onBulkPrompt?: (stage: ShotStage, shotIds: string[], promptText: string) => void | Promise<void>;
  onCreateBatch?: (stage: ShotStage, shotIds: string[]) => void | Promise<void>;
}

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
  onCreateBatch,
}: ShotBulkConfigPanelProps) {
  const [stage, setStage] = useState<ShotStage>("image");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [busyAction, setBusyAction] = useState<string>();
  const [localError, setLocalError] = useState<string>();
  const [localNotice, setLocalNotice] = useState<string>();
  const [promptText, setPromptText] = useState("");
  const shotIdKey = shots.map((shot) => shot.id).join("|");

  useEffect(() => {
    const available = new Set(shots.map((shot) => shot.id));
    setSelectedIds((current) => new Set([...current].filter((id) => available.has(id))));
  }, [shotIdKey]);

  const selectedCount = selectedIds.size;
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

  function applyPreset(preset: BulkSelectionPreset) {
    setSelectedIds(new Set(bulkSelectionIds(shots, preset, stage)));
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

  async function createBatch(nextStage: ShotStage) {
    await runAction(
      `batch-${nextStage}`,
      async (shotIds) => {
        if (onCreateBatch) {
          await onCreateBatch(nextStage, shotIds);
          return;
        }

        const plan = await planShotBatch(projectId, nextStage);
        const rows = new Map(plan.rows.map((row) => [row.shotId, row]));
        const blocked = shotIds
          .map((shotId) => rows.get(shotId))
          .filter((row) => !row || !row.eligible);
        if (blocked.length || shotIds.length > plan.maxItems) {
          const details = blocked
            .map((row) => row ? `${row.name}：${row.blockingReasons.join("；")}` : "所选镜头不在当前项目批次计划中")
            .join("；");
          const limit = shotIds.length > plan.maxItems ? `所选数量超过当前批次上限 ${plan.maxItems}。` : "";
          throw new Error([details, limit].filter(Boolean).join(" "));
        }

        const batch = await createShotBatch({ projectId, stage: nextStage, shotIds });
        try {
          await startProductionQueue(projectId, batch.id);
          reportNotice(`${stageLabel(nextStage)} 批次已创建并开始严格串行执行；结果仍需人工审核/选择。`);
        } catch (startError: unknown) {
          reportNotice(`${stageLabel(nextStage)} 批次已创建为待启动状态：${batch.name}。${toUserMessage(startError)}`);
        }
      },
      `${stageLabel(nextStage)} 批次操作已完成；生成结果仍需人工审核/选择。`,
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
        <button type="button" onClick={() => void createBatch("image")} disabled={Boolean(busyAction) || !selectedCount}>
          {busyAction === "batch-image" ? "正在创建图片批次…" : "创建图片批次"}
        </button>
        <button type="button" onClick={() => void createBatch("video")} disabled={Boolean(busyAction) || !selectedCount}>
          {busyAction === "batch-video" ? "正在创建视频批次…" : "创建视频批次"}
        </button>
      </div>

      <label className="pipeline-prompt-editor">
        <span>批量应用当前阶段提示词</span>
        <textarea value={promptText} onChange={(event) => setPromptText(event.target.value)} rows={3} placeholder="输入后点击“批量应用提示词”；文本会以当前阶段快照写入所选镜头。" disabled={Boolean(busyAction)} />
      </label>

      <div className="pipeline-shot-table-wrap">
        <table className="pipeline-shot-table">
          <thead>
            <tr><th aria-label="选择" /><th>镜头</th><th>当前提示词快照</th><th>图片状态</th><th>视频状态</th></tr>
          </thead>
          <tbody>
            {shots.map((shot) => (
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
                <td><span className="pipeline-status-chip">{statusLabel(deriveStageStatus(shot, "image"))}</span></td>
                <td><span className="pipeline-status-chip">{statusLabel(deriveStageStatus(shot, "video"))}</span></td>
              </tr>
            ))}
            {!shots.length && <tr><td colSpan={5}><p className="empty-state">当前项目还没有镜头。</p></td></tr>}
          </tbody>
        </table>
      </div>

      <p className="pipeline-human-review-note"><strong>人工审核保持不变：</strong>批量操作不会自动选择第一张图片、最新图片或第一个视频；图片生成后停在图片审核，视频生成后停在视频审核。</p>
      {localNotice && <p className="studio-notice">{localNotice}</p>}
      {localError && <p className="error-message" role="alert">{localError}</p>}
    </section>
  );
}
