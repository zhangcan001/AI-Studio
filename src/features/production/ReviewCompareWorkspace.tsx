import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type {
  ReviewCompareAction,
  ReviewCompareCandidate,
  ReviewCompareItem,
  ReviewCompareSlot,
} from "../../types/reviewProductivity";
import { REVIEW_NOTE_MAX_BYTES, reviewNoteByteLength, validateReviewNote } from "../../types/reviewProductivity";
import { ReviewCompareInspector } from "./ReviewCompareInspector";
import { ReviewCompareMedia } from "./ReviewCompareMedia";
import "./ReviewCompareWorkspace.css";

export type ReviewCompareActionHandler = (candidate: ReviewCompareCandidate, item: ReviewCompareItem) => void | Promise<void>;
export type ReviewCompareNoteHandler = (item: ReviewCompareItem, note: string) => void | Promise<void>;

export interface ReviewCompareWorkspaceProps {
  items: readonly ReviewCompareItem[];
  currentItemId?: string;
  busy?: boolean;
  error?: string;
  actionAvailability?: Partial<Record<ReviewCompareAction, boolean>>;
  onItemChange?: (item: ReviewCompareItem) => void;
  onBeforeItemChange?: (from: ReviewCompareItem, to: ReviewCompareItem, dirtyNote: string) => boolean | Promise<boolean>;
  onConfirmAndApprove?: ReviewCompareActionHandler;
  onApprove?: ReviewCompareActionHandler;
  onStar?: ReviewCompareActionHandler;
  onReject?: ReviewCompareActionHandler;
  onRegenerate?: ReviewCompareActionHandler;
  onCreateReworkBatch?: ReviewCompareActionHandler;
  onSaveNote?: ReviewCompareNoteHandler;
}

const REVIEW_STATUS_LABELS: Record<string, string> = {
  UNREVIEWED: "未审",
  APPROVED: "通过",
  STARRED: "已标星",
  REGENERATE: "待返工",
  REJECTED: "已拒绝",
  FAILED: "失败",
  IN_PROGRESS: "生成中",
};

const FAILURE_STATUSES = new Set(["FAILED", "ERROR", "CANCELLED", "SKIPPED", "PENDING", "DISPATCHING", "DISPATCHED", "IN_PROGRESS", "RUNNING"]);

function candidateTitle(candidate: ReviewCompareCandidate): string {
  return candidate.label ?? (candidate.version ? `V${candidate.version}` : candidate.asset.name);
}

function candidateIsSuccessful(candidate: ReviewCompareCandidate | undefined): boolean {
  if (!candidate) return false;
  if (candidate.productionItemStatus !== undefined) return candidate.productionItemStatus === "SUCCEEDED";
  if (candidate.taskStatus !== undefined) return !FAILURE_STATUSES.has(candidate.taskStatus.toUpperCase());
  return true;
}

function itemIsReviewable(item: ReviewCompareItem): boolean {
  return item.reviewStatus !== "FAILED" && item.reviewStatus !== "IN_PROGRESS";
}

function statusLabel(candidate: ReviewCompareCandidate, item: ReviewCompareItem): string {
  if (candidate.selected || item.selectedCandidateId === candidate.id) return "当前选择";
  return candidate.reviewStatus ? REVIEW_STATUS_LABELS[candidate.reviewStatus] ?? candidate.reviewStatus : item.reviewStatus ? REVIEW_STATUS_LABELS[item.reviewStatus] ?? item.reviewStatus : "候选结果";
}

export function ReviewCompareWorkspace({
  items,
  currentItemId,
  busy = false,
  error,
  actionAvailability,
  onItemChange,
  onBeforeItemChange,
  onConfirmAndApprove,
  onApprove,
  onStar,
  onReject,
  onRegenerate,
  onCreateReworkBatch,
  onSaveNote,
}: ReviewCompareWorkspaceProps) {
  const [internalItemId, setInternalItemId] = useState(items[0]?.id);
  const [focusedSlot, setFocusedSlot] = useState<ReviewCompareSlot>("A");
  const [slotSelections, setSlotSelections] = useState<Record<string, Partial<Record<ReviewCompareSlot, string>>>>({});
  const [noteDrafts, setNoteDrafts] = useState<Record<string, string>>({});
  const [noteError, setNoteError] = useState<string>();
  const slotRefs = useRef<Partial<Record<ReviewCompareSlot, HTMLElement | null>>>({});

  const activeItemId = currentItemId ?? internalItemId;
  const activeIndex = Math.max(0, items.findIndex((item) => item.id === activeItemId));
  const activeItem = items[activeIndex];

  useEffect(() => {
    if (currentItemId) return;
    if (items.some((item) => item.id === internalItemId)) return;
    setInternalItemId(items[0]?.id);
  }, [currentItemId, internalItemId, items]);

  useEffect(() => {
    setNoteDrafts((current) => {
      const next = { ...current };
      for (const item of items) if (!(item.id in next)) next[item.id] = item.reviewNote ?? "";
      return next;
    });
  }, [items]);

  const selection = useMemo(() => {
    if (!activeItem) return { A: undefined, B: undefined };
    const stored = slotSelections[activeItem.id] ?? {};
    return {
      A: activeItem.candidates.find((candidate) => candidate.id === stored.A) ?? activeItem.candidates[0],
      B: activeItem.candidates.find((candidate) => candidate.id === stored.B) ?? activeItem.candidates[1],
    };
  }, [activeItem, slotSelections]);

  const setSlot = (slot: ReviewCompareSlot, candidate: ReviewCompareCandidate) => {
    if (!activeItem) return;
    setFocusedSlot(slot);
    setSlotSelections((current) => ({ ...current, [activeItem.id]: { ...(current[activeItem.id] ?? {}), [slot]: candidate.id } }));
  };

  const swapSlots = () => {
    if (!activeItem || !selection.A || !selection.B) return;
    setSlotSelections((current) => ({ ...current, [activeItem.id]: { A: selection.B?.id, B: selection.A?.id } }));
  };

  async function confirmItemChange(nextItem: ReviewCompareItem | undefined) {
    if (!nextItem || !activeItem || nextItem.id === activeItem.id) return;
    const note = noteDrafts[activeItem.id] ?? activeItem.reviewNote ?? "";
    const savedNote = activeItem.reviewNote ?? "";
    if (note !== savedNote) {
      const allowed = onBeforeItemChange
        ? await onBeforeItemChange(activeItem, nextItem, note)
        : typeof window !== "undefined" && typeof window.confirm === "function"
          ? window.confirm("当前备注尚未保存，确定切换审核项吗？")
          : false;
      if (!allowed) return;
    }
    setFocusedSlot("A");
    setNoteError(undefined);
    if (currentItemId === undefined) setInternalItemId(nextItem.id);
    onItemChange?.(nextItem);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (!activeItem) return;
    if (event.key === "ArrowRight") {
      event.preventDefault();
      void confirmItemChange(items[activeIndex + 1]);
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      void confirmItemChange(items[activeIndex - 1]);
    } else if (event.key === "1") {
      event.preventDefault();
      setFocusedSlot("A");
      slotRefs.current.A?.focus();
    } else if (event.key === "2" && selection.B) {
      event.preventDefault();
      setFocusedSlot("B");
      slotRefs.current.B?.focus();
    } else if ((event.key === "Enter" || event.key === " ") && event.target === event.currentTarget) {
      // The workspace keyboard contract deliberately has no implicit approve,
      // selection, or regeneration action.
      event.preventDefault();
    }
  }

  function can(action: ReviewCompareAction, handler: unknown): boolean {
    return !busy && actionAvailability?.[action] !== false && Boolean(handler);
  }

  function runAction(handler: ReviewCompareActionHandler | undefined) {
    if (!activeItem || !handler) return;
    const candidate = selection[focusedSlot] ?? selection.A;
    if (!candidate || !candidateIsSuccessful(candidate) || !itemIsReviewable(activeItem)) return;
    void handler(candidate, activeItem);
  }

  function saveNote() {
    if (!activeItem || !onSaveNote) return;
    const note = noteDrafts[activeItem.id] ?? "";
    const validationError = validateReviewNote(note);
    setNoteError(validationError);
    if (validationError) return;
    void onSaveNote(activeItem, note);
  }

  if (!activeItem) {
    return <section className="review-compare-workspace" aria-label="候选结果对比"><p className="empty-state">当前没有可审核项。</p></section>;
  }

  const activeCandidate = selection[focusedSlot] ?? selection.A;
  const successful = candidateIsSuccessful(activeCandidate) && itemIsReviewable(activeItem);
  const draftNote = noteDrafts[activeItem.id] ?? activeItem.reviewNote ?? "";
  const noteBytes = reviewNoteByteLength(draftNote);
  const noteDirty = draftNote !== (activeItem.reviewNote ?? "");
  const setFinalResultAvailable = Boolean(activeItem.shotId) && successful && actionAvailability?.confirmAndApprove !== false;

  return (
    <section
      className="review-compare-workspace"
      aria-label="候选结果对比工作区"
      tabIndex={0}
      aria-keyshortcuts="ArrowLeft ArrowRight 1 2"
      onKeyDown={handleKeyDown}
    >
      <header className="review-compare-header">
        <div>
          <span className="section-label">人工复核</span>
          <h2>{activeItem.shotName ?? activeItem.name ?? `审核项 ${activeItem.ordinal + 1}`}</h2>
          <p>候选比较只改变 A/B 槽位；不会选择 Shot，也不会隐式批准或生成。</p>
        </div>
        <div className="review-compare-navigation" aria-label="审核项导航">
          <button type="button" className="quiet-button" onClick={() => void confirmItemChange(items[activeIndex - 1])} disabled={busy || activeIndex <= 0}>上一项</button>
          <span>{activeIndex + 1} / {items.length}</span>
          <button type="button" className="quiet-button" onClick={() => void confirmItemChange(items[activeIndex + 1])} disabled={busy || activeIndex >= items.length - 1}>下一项</button>
        </div>
      </header>

      {error && <p className="review-compare-error" role="alert">{error}</p>}

      <div className="review-compare-candidate-strip" aria-label="候选结果条">
        <div className="review-compare-strip-heading"><strong>候选（{activeItem.candidates.length}）</strong><span>点击仅放入当前 A/B 槽位</span></div>
        <div className="review-compare-candidate-list">
          {activeItem.candidates.map((candidate) => (
            <div key={candidate.id} className={`review-compare-candidate${candidate.selected || activeItem.selectedCandidateId === candidate.id ? " selected" : ""}`}>
              <button type="button" className="review-compare-candidate-main" onClick={() => setSlot(focusedSlot, candidate)} aria-label={`将 ${candidateTitle(candidate)} 放入 ${focusedSlot} 槽位`}>
                <span className="review-compare-candidate-index">{candidateTitle(candidate)}</span>
                <span>{statusLabel(candidate, activeItem)}</span>
              </button>
              {activeItem.candidates.length > 1 && <div className="review-compare-candidate-slot-buttons">
                <button type="button" aria-label={`将 ${candidateTitle(candidate)} 放入 A 槽位`} aria-pressed={selection.A?.id === candidate.id} onClick={() => setSlot("A", candidate)}>A</button>
                <button type="button" aria-label={`将 ${candidateTitle(candidate)} 放入 B 槽位`} aria-pressed={selection.B?.id === candidate.id} onClick={() => setSlot("B", candidate)}>B</button>
              </div>}
            </div>
          ))}
        </div>
      </div>

      <div className={`review-compare-panels${selection.B ? " has-two" : " single"}`}>
        <ComparePanel
          slot="A"
          candidate={selection.A}
          selected={selection.A ? selection.A.selected || activeItem.selectedCandidateId === selection.A.id : false}
          focused={focusedSlot === "A"}
          onFocus={() => setFocusedSlot("A")}
          onRef={(element) => { slotRefs.current.A = element; }}
        />
        {selection.B && <ComparePanel
          slot="B"
          candidate={selection.B}
          selected={selection.B.selected || activeItem.selectedCandidateId === selection.B.id}
          focused={focusedSlot === "B"}
          onFocus={() => setFocusedSlot("B")}
          onRef={(element) => { slotRefs.current.B = element; }}
        />}
        {selection.B && <button type="button" className="review-compare-swap" onClick={swapSlots} disabled={busy} aria-label="交换 A/B 槽位">交换 A/B</button>}
      </div>

      <div className="review-compare-lower-grid">
        <ReviewCompareInspector item={activeItem} candidate={activeCandidate} />
        <section className="review-compare-notes" aria-label="审核备注">
          <div className="review-compare-section-title"><div><span className="section-label">审核记录</span><h3>备注</h3></div><span className={noteBytes > REVIEW_NOTE_MAX_BYTES ? "review-compare-note-count over" : "review-compare-note-count"}>{noteBytes} / {REVIEW_NOTE_MAX_BYTES} bytes</span></div>
          <textarea
            value={draftNote}
            maxLength={REVIEW_NOTE_MAX_BYTES}
            rows={5}
            aria-label="审核备注"
            placeholder="记录画面、节奏或返工原因……"
            disabled={busy}
            onChange={(event) => {
              const next = event.target.value;
              setNoteDrafts((current) => ({ ...current, [activeItem.id]: next }));
              setNoteError(validateReviewNote(next));
            }}
          />
          {noteError && <p className="review-compare-field-error" role="alert">{noteError}</p>}
          {noteDirty && <p className="review-compare-dirty-note">备注未保存；切换审核项时会先提示。</p>}
          <button type="button" className="quiet-button" onClick={saveNote} disabled={!can("saveNote", onSaveNote) || Boolean(noteError)}>保存备注</button>
        </section>
      </div>

      <div className="review-compare-actions" aria-label="审核动作">
        {setFinalResultAvailable && <button type="button" onClick={() => runAction(onConfirmAndApprove)} disabled={!can("confirmAndApprove", onConfirmAndApprove)}>确认并通过</button>}
        <button type="button" onClick={() => runAction(onApprove)} disabled={!can("approve", onApprove) || !successful}>仅通过</button>
        <button type="button" className="review-star-button" onClick={() => runAction(onStar)} disabled={!can("star", onStar) || !successful}>标星</button>
        <button type="button" className="quiet-button danger-button" onClick={() => runAction(onReject)} disabled={!can("reject", onReject) || !successful}>拒绝</button>
        <button type="button" className="quiet-button" onClick={() => runAction(onRegenerate)} disabled={!can("regenerate", onRegenerate) || !successful}>标记返工</button>
        <button type="button" className="quiet-button" onClick={() => runAction(onCreateReworkBatch)} disabled={!can("createReworkBatch", onCreateReworkBatch) || !successful}>创建返工批次</button>
      </div>
    </section>
  );
}

function ComparePanel({
  slot,
  candidate,
  selected,
  focused,
  onFocus,
  onRef,
}: {
  slot: ReviewCompareSlot;
  candidate?: ReviewCompareCandidate;
  selected: boolean;
  focused: boolean;
  onFocus: () => void;
  onRef: (element: HTMLElement | null) => void;
}) {
  return (
    <article
      ref={onRef}
      className={`review-compare-panel${focused ? " focused" : ""}`}
      tabIndex={-1}
      data-slot={slot}
      data-focused={focused ? "true" : "false"}
      onClick={onFocus}
      aria-label={`${slot} 槽位`}
    >
      <div className="review-compare-panel-heading"><strong>{slot}</strong><span>{candidate ? candidateTitle(candidate) : "暂无候选"}</span>{selected && <em>当前选择</em>}</div>
      {candidate ? <ReviewCompareMedia asset={candidate.asset} mediaKind={candidate.mediaKind} imageUrl={candidate.imageUrl} mediaUrl={candidate.mediaUrl} label={`${slot} ${candidateTitle(candidate)}`} /> : <div className="review-compare-media-empty"><span>暂无候选</span></div>}
    </article>
  );
}
