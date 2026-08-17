import { useEffect, useRef, useState } from "react";
import {
  getProductionPartialResumePlan,
  partialResumeProductionQueue,
  startProductionQueue,
} from "../../services/tauriClient";
import type {
  ProductionBatchDetail,
  ProductionPartialResumeEntry,
  ProductionPartialResumePlan,
} from "../../types/productionQueue";
import { formatUiError, toUserMessage } from "../../i18n/errorMessages";

const QUEUE_BUSY_NOTICE = "恢复任务已准备完成，当前生产队列繁忙，可稍后启动。";

export function defaultPartialResumeSelection(plan: ProductionPartialResumePlan): string[] {
  return plan.entries
    .filter((entry) => entry.eligibility === "AUTO_RESUMABLE")
    .map((entry) => entry.leafItemId);
}

interface PartialResumePreviewProps {
  plan: ProductionPartialResumePlan;
  selectedLeafItemIds: string[];
  onToggle: (leafItemId: string) => void;
  onConfirm: () => void;
  busy: boolean;
  disabled?: boolean;
}

export function PartialResumePreview({
  plan,
  selectedLeafItemIds,
  onToggle,
  onConfirm,
  busy,
  disabled = false,
}: PartialResumePreviewProps) {
  const entries = plan.entries.filter(
    (entry) => entry.eligibility === "AUTO_RESUMABLE" || entry.eligibility === "REVIEW_REQUIRED",
  );
  const canSelect = plan.canResume && !busy && !disabled;

  return (
    <div className="production-partial-resume-preview">
      <div className="production-partial-resume-heading">
        <div>
          <span className="section-label">失败项恢复</span>
          <p>只恢复已确认安全的失败项，原始任务和输入保持不变。</p>
        </div>
      </div>
      <div className="production-partial-resume-stats" aria-label="失败项恢复统计">
        <span>逻辑任务：<strong>{plan.logicalTotal}</strong></span>
        <span>已完成：<strong>{plan.resolved}</strong></span>
        <span>可恢复：<strong>{plan.autoResumable}</strong></span>
        <span>需人工检查：<strong>{plan.reviewRequired}</strong></span>
        <span>Attempts：<strong>{plan.attemptTotal}</strong></span>
      </div>
      {(plan.pending > 0 || plan.active > 0 || !plan.canResume) && (
        <p className="disabled-note">
          {plan.pending > 0 || plan.active > 0
            ? "当前批次仍有等待中或执行中的项目，完成后再进行局部恢复。"
            : "当前批次暂时不能进行局部恢复。"}
        </p>
      )}
      {entries.length > 0 ? (
        <ul className="production-partial-resume-list" aria-label="失败项恢复预览">
          {entries.map((entry) => (
            <PartialResumeEntryRow
              key={entry.leafItemId}
              entry={entry}
              checked={selectedLeafItemIds.includes(entry.leafItemId)}
              disabled={!canSelect || entry.eligibility !== "AUTO_RESUMABLE"}
              onToggle={onToggle}
            />
          ))}
        </ul>
      ) : (
        <p className="disabled-note">当前批次没有可恢复的失败项。</p>
      )}
      <div className="production-partial-resume-actions">
        <span>已选择 {selectedLeafItemIds.length} 项</span>
        <button
          type="button"
          onClick={onConfirm}
          disabled={busy || disabled || !plan.canResume || selectedLeafItemIds.length === 0}
        >
          {busy ? "正在恢复…" : "恢复选中项"}
        </button>
      </div>
    </div>
  );
}

function PartialResumeEntryRow({
  entry,
  checked,
  disabled,
  onToggle,
}: {
  entry: ProductionPartialResumeEntry;
  checked: boolean;
  disabled: boolean;
  onToggle: (leafItemId: string) => void;
}) {
  const reviewRequired = entry.eligibility === "REVIEW_REQUIRED";
  const error = [entry.errorCode, entry.errorMessage].filter(Boolean).join("：");
  return (
    <li className={`production-partial-resume-entry${reviewRequired ? " review-required" : ""}`}>
      <label>
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={() => onToggle(entry.leafItemId)}
        />
        <span>
          <strong>第 {entry.ordinal + 1} 项</strong>
          <small>{reviewRequired ? "需人工检查" : "可自动恢复"} · attempts {entry.attemptCount}</small>
          {error && <small title={entry.errorMessage}>{error}</small>}
        </span>
      </label>
    </li>
  );
}

interface Props {
  projectId: string;
  batchId: string;
  disabled?: boolean;
  onBatchChanged?: (detail: ProductionBatchDetail) => void | Promise<void>;
}

export function ProductionPartialResumePanel({
  projectId,
  batchId,
  disabled = false,
  onBatchChanged,
}: Props) {
  const [expanded, setExpanded] = useState(false);
  const [plan, setPlan] = useState<ProductionPartialResumePlan>();
  const [selectedLeafItemIds, setSelectedLeafItemIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string>();
  const requestVersion = useRef(0);
  const resumeInFlight = useRef(false);

  useEffect(() => {
    requestVersion.current += 1;
    setExpanded(false);
    setPlan(undefined);
    setSelectedLeafItemIds([]);
    setLoading(false);
    setBusy(false);
    setNotice(undefined);
    resumeInFlight.current = false;
  }, [batchId]);

  async function loadPlan(clearNotice = true) {
    const version = ++requestVersion.current;
    setLoading(true);
    if (clearNotice) setNotice(undefined);
    try {
      const next = await getProductionPartialResumePlan(projectId, batchId);
      if (version !== requestVersion.current) return;
      setPlan(next);
      setSelectedLeafItemIds(defaultPartialResumeSelection(next));
    } catch (error: unknown) {
      if (version === requestVersion.current) setNotice(toUserMessage(error));
    } finally {
      if (version === requestVersion.current) setLoading(false);
    }
  }

  function toggleExpanded() {
    const nextExpanded = !expanded;
    setExpanded(nextExpanded);
    if (nextExpanded && !plan && !loading) void loadPlan();
  }

  function toggleSelection(leafItemId: string) {
    setSelectedLeafItemIds((current) => current.includes(leafItemId)
      ? current.filter((id) => id !== leafItemId)
      : [...current, leafItemId]);
  }

  async function notifyBatchChanged(detail: ProductionBatchDetail) {
    try {
      await onBatchChanged?.(detail);
    } catch {
      // The queue mutation is already persisted; a later refresh can recover the view.
    }
  }

  async function confirmResume() {
    if (resumeInFlight.current || busy || disabled || !plan?.canResume || !selectedLeafItemIds.length) return;
    resumeInFlight.current = true;
    setBusy(true);
    setNotice(undefined);
    try {
      const result = await partialResumeProductionQueue(projectId, batchId, selectedLeafItemIds);
      await notifyBatchChanged(result.detail);

      if (result.createdCount > 0 || result.alreadyPreparedCount > 0) {
        try {
          const started = await startProductionQueue(projectId, batchId);
          await notifyBatchChanged(started);
          setNotice("恢复任务已准备完成并开始生产。");
        } catch (error: unknown) {
          await notifyBatchChanged(result.detail);
          if (formatUiError(error).code === "PRODUCTION_QUEUE_BUSY") {
            setNotice(QUEUE_BUSY_NOTICE);
          } else {
            setNotice(`恢复任务已准备完成，但启动失败：${toUserMessage(error)}`);
          }
        }
      } else {
        setNotice("没有新增恢复任务，所选项已经准备完成。");
      }
      await loadPlan(false);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      resumeInFlight.current = false;
      setBusy(false);
    }
  }

  return (
    <section className="production-partial-resume" aria-label="失败项恢复">
      <button
        type="button"
        className="production-partial-resume-toggle quiet-button"
        aria-expanded={expanded}
        onClick={toggleExpanded}
        disabled={disabled}
      >
        {expanded ? "收起失败项恢复" : "恢复失败项"}
      </button>
      {expanded && (
        loading && !plan
          ? <p className="disabled-note">正在读取失败项恢复计划…</p>
          : plan && (
            <PartialResumePreview
              plan={plan}
              selectedLeafItemIds={selectedLeafItemIds}
              onToggle={toggleSelection}
              onConfirm={() => void confirmResume()}
              busy={busy}
              disabled={disabled}
            />
          )
      )}
      {notice && <p className="disabled-note" role="status" aria-live="polite">{notice}</p>}
    </section>
  );
}
