import { useEffect, useMemo, useState } from "react";
import {
  createShotBatch,
  planShotBatch,
  startProductionQueue,
} from "../../services/tauriClient";
import type { ShotBatchPlan, ShotStage, ShotView } from "../../types/shot";
import { toUserMessage } from "../../i18n/errorMessages";
import { shotStatusLabels } from "./shotDomain";
import { stageLabel } from "./shotBatchDomain";

interface Props {
  projectId: string;
  shots: ShotView[];
  onRefresh: () => Promise<void>;
  onNotice: (message: string) => void;
  onError: (message?: string) => void;
}

export function ShotBatchPlanner({ projectId, shots, onRefresh, onNotice, onError }: Props) {
  const [open, setOpen] = useState(false);
  const [stage, setStage] = useState<ShotStage>("image");
  const [plan, setPlan] = useState<ShotBatchPlan>();
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);

  const loadPlan = async () => {
    setLoading(true);
    onError(undefined);
    try {
      const next = await planShotBatch(projectId, stage);
      setPlan(next);
      setSelectedIds((current) => new Set([...current].filter((id) => next.rows.some((row) => row.shotId === id && row.eligible))));
    } catch (error: unknown) {
      onError(toUserMessage(error));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open) void loadPlan();
    // The planner is intentionally refreshed only when opened or when the
    // user changes stage; explicit refresh keeps the panel predictable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, stage, projectId]);

  const eligibleIds = useMemo(
    () => plan?.rows.filter((row) => row.eligible).map((row) => row.shotId) ?? [],
    [plan],
  );
  const allEligibleSelected = shots.length > 0 && eligibleIds.length > 0 && eligibleIds.every((id) => selectedIds.has(id));

  function toggleShot(shotId: string) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(shotId)) next.delete(shotId);
      else if (next.size < (plan?.maxItems ?? 100)) next.add(shotId);
      return next;
    });
  }

  function toggleAll() {
    setSelectedIds(allEligibleSelected ? new Set() : new Set(eligibleIds.slice(0, plan?.maxItems ?? 100)));
  }

  async function createAndStart() {
    if (!selectedIds.size) return;
    setCreating(true);
    onError(undefined);
    try {
      const detail = await createShotBatch({ projectId, stage, shotIds: [...selectedIds] });
      try {
        await startProductionQueue(projectId, detail.id);
        onNotice(`${stageLabel(stage)}批次已创建并开始严格串行执行；结果仍需逐个手动确认。`);
      } catch (startError: unknown) {
        onNotice(`批次已创建为待启动状态（${detail.name}）。${toUserMessage(startError)} 可在生产队列中继续。`);
      }
      setSelectedIds(new Set());
      await onRefresh();
      await loadPlan();
    } catch (error: unknown) {
      onError(toUserMessage(error));
    } finally {
      setCreating(false);
    }
  }

  return (
    <section className={`shot-batch-panel${open ? " shot-batch-panel-open" : ""}`} aria-label="批量 Shot 生产">
      <div className="shot-batch-panel-heading">
        <div>
          <span className="section-label">批量规划</span>
          <h3>批量生产与复核</h3>
          <p className="shot-inline-note">先批量生成关键帧，再由你逐个选定；点击视频批次后才会进入 MiniMax H3 队列。</p>
        </div>
        <button type="button" className={open ? "quiet-button" : "shot-primary-action"} onClick={() => setOpen((current) => !current)}>
          {open ? "收起批量规划" : "打开批量规划"}
        </button>
      </div>
      {open && (
        <>
          <div className="shot-batch-stage-tabs" role="tablist" aria-label="批量阶段">
            {(["image", "video"] as const).map((nextStage) => (
              <button key={nextStage} type="button" className={stage === nextStage ? "active" : ""} onClick={() => setStage(nextStage)} role="tab" aria-selected={stage === nextStage}>
                {nextStage === "image" ? "Kera2 · 批量关键帧" : "MiniMax H3 · 批量视频"}
              </button>
            ))}
            <button type="button" className="quiet-button" onClick={() => void loadPlan()} disabled={loading || creating}>重新检查资格</button>
          </div>
          <div className="shot-batch-frozen-notice">
            <strong>批次快照规则</strong>
            <span>入队时冻结每个 Shot 的 Prompt、scalar、Reference、工作流版本与 Recipe；之后编辑 Shot 不会改写已入队项。</span>
          </div>
          <div className="shot-batch-selection-bar">
            <label><input type="checkbox" checked={allEligibleSelected} onChange={toggleAll} disabled={!eligibleIds.length || loading || creating} /> 全选符合条件的镜头</label>
            <span>{selectedIds.size} / {plan?.maxItems ?? 100} 已选择 · {plan?.eligibleCount ?? 0} 个符合条件</span>
            <button type="button" className="shot-primary-action" onClick={() => void createAndStart()} disabled={creating || loading || !selectedIds.size}>
              {creating ? "正在创建队列…" : stage === "image" ? "批量生成关键帧" : "批量生成视频（MiniMax H3）"}
            </button>
          </div>
          {loading ? <p className="project-loading">正在检查当前工作流、素材与任务状态…</p> : (
            <div className="shot-batch-table-wrap">
              <table className="shot-batch-table">
                <thead><tr><th aria-label="选择" /><th>Shot</th><th>当前状态</th><th>工作流 / Recipe</th><th>资格与阻塞原因</th></tr></thead>
                <tbody>
                  {(plan?.rows ?? []).map((row) => (
                    <tr key={row.shotId} className={row.eligible ? "" : "shot-batch-row-blocked"}>
                      <td><input type="checkbox" checked={selectedIds.has(row.shotId)} onChange={() => toggleShot(row.shotId)} disabled={!row.eligible || creating} aria-label={`选择${row.name}`} /></td>
                      <td><strong>{String(row.ordinal + 1).padStart(2, "0")} · {row.name}</strong><small>{stageLabel(stage)}</small></td>
                      <td><span className={`shot-status-chip shot-status-${row.currentStatus.toLowerCase()}`}>{shotStatusLabels[row.currentStatus as keyof typeof shotStatusLabels] ?? row.currentStatus}</span></td>
                      <td><strong>{row.recipeName ?? "未配置"}</strong><small>{row.workflowVersionId ? "当前版本 · 入队时冻结" : "等待配置"}</small></td>
                      <td>{row.eligible ? <span className="shot-batch-ready">可加入批次</span> : <span className="shot-batch-reasons">{row.blockingReasons.join("；")}</span>}</td>
                    </tr>
                  ))}
                  {!plan?.rows.length && <tr><td colSpan={5}><p className="empty-state">当前项目还没有 Shot。</p></td></tr>}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </section>
  );
}
