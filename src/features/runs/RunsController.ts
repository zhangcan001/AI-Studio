import { useCallback, useEffect, useRef, useState } from "react";
import type { AppRoute } from "../../app/routes/types";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns, subscribeRunInvalidation } from "../../product/runInvalidation";
import { runRecoveryPresentation } from "./runRecoveryPresentation";
import type { ProductRun, RunInput, RunList, RunResult } from "../../product/types";
import { useStudioStore } from "../../stores/studioStore";
import { filterFor, type RunsRoute } from "./runsModel";
export interface RunsProps { route: RunsRoute; navigate: (route: AppRoute) => unknown }
export function useRunsController({ route, navigate }: RunsProps) {
  const [list, setList] = useState<RunList>();
  const [detail, setDetail] = useState<ProductRun>();
  const [results, setResults] = useState<RunResult[]>([]);
  const [error, setError] = useState<string>();
  const [actionError, setActionError] = useState<string>();
  const [missing, setMissing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const epoch = useRef(0); const actionLock = useRef(false);
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  // Single-flight per scope: a slow IPC round trip (> poll interval) must not be superseded forever by polls.
  const inflight = useRef<number | undefined>(undefined); const queued = useRef(false); const rerun = useRef<(() => Promise<void>) | undefined>(undefined);
  const filter = filterFor(route);
  const scope = `${route.projectId}:${route.run?.source ?? ""}:${route.run?.id ?? ""}:${filter}`;
  const liveScope = useRef(scope); liveScope.current = scope;
  const refresh = useCallback(async () => {
    if (inflight.current === epoch.current) { queued.current = true; return; }
    const token = ++epoch.current; inflight.current = token; queued.current = false;
    try {
      const issues: string[] = [];
      // Recent history and a directly addressed Run are independent reads.
      // A broken list must not suppress the selected Run's authoritative state.
      await Promise.all([
        (async () => {
          try {
            const page = await productClient.run.list(route.projectId, filter, null);
            if (token === epoch.current) setList(page);
          } catch (cause) { issues.push(normalizeProductError(cause).message); }
        })(),
        (async () => {
          if (!route.run) return;
          try {
            const run = await productClient.run.get(route.projectId, route.run);
            if (token !== epoch.current) return;
            // Commit known state before fetching optional results: never retain
            // stale READY actions when resultsGet fails after a RUNNING read.
            setDetail(run); setMissing(false);
            const outputs = await productClient.run.resultsGet(route.projectId, route.run);
            if (token === epoch.current) setResults(outputs);
          } catch (cause) {
            if (token !== epoch.current) return;
            const issue = normalizeProductError(cause);
            setResults([]);
            if (issue.code === "RUN_NOT_FOUND" || issue.code === "PROJECT_SCOPE_VIOLATION") {
              setMissing(true); setDetail(undefined);
            } else { issues.push(issue.message); }
          }
        })(),
      ]);
      if (token === epoch.current) setError(issues.length ? issues.join("；") : undefined);
    }
    finally {
      if (token === epoch.current) {
        setLoading(false); inflight.current = undefined;
        if (queued.current) { queued.current = false; void rerun.current?.(); }
      }
    }
  }, [route.projectId, route.run?.source, route.run?.id, filter]);
  rerun.current = refresh;
  useEffect(() => {
    setList(undefined); setDetail(undefined); setResults([]); setMissing(false); setError(undefined); setActionError(undefined); setLoading(true);
    void refresh();
    let notification: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = subscribeRunInvalidation(projectId => {
      if (projectId !== route.projectId) return;
      clearTimeout(notification);
      notification = setTimeout(() => void refresh(), 200);
    });
    // Fallback for authorities without their own batch/review event channel.
    const timer = setInterval(() => { if (document.visibilityState !== "hidden") void refresh(); }, 5000);
    return () => { ++epoch.current; unsubscribe(); clearTimeout(notification); clearInterval(timer); };
  }, [refresh]);
  async function mutate(action: () => Promise<unknown>) {
    if (actionLock.current) return false;
    actionLock.current = true; setBusy(true); setActionError(undefined); const owner = scope;
    try { await action(); invalidateRuns(route.projectId); return true; }
    catch (error) { if (owner === liveScope.current) setActionError(normalizeProductError(error).message); return false; }
    finally { actionLock.current = false; setBusy(false); }
  }
  const action = (kind: "START" | "PAUSE" | "CANCEL" | "RETRY") => mutate(async () => {
    if (!detail?.availableActions.includes(kind)) return;
    if (kind === "CANCEL" && detail.status === "RUNNING" && !window.confirm("确定取消这个运行吗？正在生成的视频会被中断，未开始的镜头不会再生成；已生成的视频和素材会保留。")) return;
    if (kind === "RETRY") {
      const confirm = runRecoveryPresentation(detail).retry?.confirm;
      if (confirm && !window.confirm(confirm)) return;
      const prepared = await productClient.run.retry(route.projectId, { ref: detail.ref, selectedItemIds: detail.recoverability.retryItemIds });
      if (mounted.current && scope === liveScope.current && prepared.projectId === route.projectId
        && (prepared.ref.source !== detail.ref.source || prepared.ref.id !== detail.ref.id)) {
        await navigate({ ...route, run: prepared.ref });
      }
    }
    else await productClient.run[kind === "START" ? "start" : kind === "PAUSE" ? "pause" : "cancel"](route.projectId, detail.ref);
  });
  const remove = () => mutate(async () => {
    if (!detail || !["SUCCEEDED", "FAILED", "PARTIAL", "CANCELLED"].includes(detail.status) || detail.ref.source === "production-run") return;
    if (!window.confirm("确定删除这条运行记录吗？已生成的视频和素材会保留。")) return;
    await productClient.run.delete(route.projectId, detail.ref);
    if (mounted.current && scope === liveScope.current) navigate({ ...route, run: undefined });
  });
  const retryAndStart = () => mutate(async () => {
    if (!detail?.availableActions.includes("RETRY") || detail.ref.source === "production-run" || !detail.recoverability.retryItemIds.length) return;
    const pendingCount = (detail.detail?.inputs ?? []).filter(item => item.executionStatus === "PENDING" || item.executionStatus === "DISPATCHING").length;
    const msg = pendingCount > 0
      ? `将按原快照重新准备失败项并立即启动当前批次（批次内其余 ${pendingCount} 个待执行项也会继续派发），已完成结果会保留。确定重试并立即启动？`
      : "将按原快照重新准备失败项并立即启动生产队列，已完成结果会保留。确定重试并立即启动？";
    if (!window.confirm(msg)) return;
    const prepared = await productClient.run.retry(route.projectId, { ref: detail.ref, selectedItemIds: detail.recoverability.retryItemIds });
    if (!mounted.current || scope !== liveScope.current || prepared.projectId !== route.projectId) return;
    if (prepared.ref.source !== detail.ref.source || prepared.ref.id !== detail.ref.id) {
      await navigate({ ...route, run: prepared.ref });
    }
    await productClient.run.start(route.projectId, prepared.ref);
  });
  const retryInput = (input: RunInput) => mutate(async () => {
    if (!detail || detail.projectId !== route.projectId || detail.ref.source !== "queue-batch"
      || detail.ref.id !== route.run?.id || route.run?.source !== "queue-batch"
      || !detail.availableActions.includes("RETRY") || !input.itemId
      || !detail.recoverability.retryItemIds.includes(input.itemId)
      || !detail.detail?.inputs.some(item => item.itemId === input.itemId && item.taskId === input.taskId)) return;
    const confirm = runRecoveryPresentation(detail).retry?.confirm;
    if (confirm && !window.confirm(confirm)) return;
    await productClient.run.retry(route.projectId, { ref: detail.ref, selectedItemIds: [input.itemId] });
  });
  const retryInputAndStart = (input: RunInput) => mutate(async () => {
    if (!detail || detail.projectId !== route.projectId || detail.ref.source !== "queue-batch"
      || detail.ref.id !== route.run?.id || route.run?.source !== "queue-batch"
      || !detail.availableActions.includes("RETRY") || !input.itemId
      || !detail.recoverability.retryItemIds.includes(input.itemId)
      || !detail.detail?.inputs.some(item => item.itemId === input.itemId && item.taskId === input.taskId)) return;
    const pendingSiblings = (detail.detail?.inputs ?? []).filter(item => item.itemId !== input.itemId && (item.executionStatus === "PENDING" || item.executionStatus === "DISPATCHING")).length;
    const confirmMessage = pendingSiblings > 0
      ? `将准备此镜头的原快照重试并启动整个队列批次；当前批次中其余 ${pendingSiblings} 个未开始的待执行镜头也会按顺序继续生成，已完成结果保持不变。若只想准备此镜头而不启动其余待执行项，请取消并点「仅重试此镜头」。确定继续？`
      : detail.status === "PAUSED"
        ? "将准备此镜头的原快照重试并恢复启动当前队列批次（批次内如有其他待执行项也会继续派发），已完成结果保持不变。确定继续？"
        : "将准备此镜头的原快照重试并立即启动当前队列批次，已完成结果保持不变。确定继续？";
    if (!window.confirm(confirmMessage)) return;
    const prepared = await productClient.run.retry(route.projectId, { ref: detail.ref, selectedItemIds: [input.itemId] });
    if (!mounted.current || scope !== liveScope.current || prepared.projectId !== route.projectId) return;
    await productClient.run.start(route.projectId, prepared.ref);
  });
  const openShot = (input: RunInput) => {
    if (!input.shotId || detail?.projectId !== route.projectId || detail.ref.id !== route.run?.id || detail.ref.source !== route.run?.source
      || !detail.detail?.sources.some(source => source.id === input.shotId && source.stage === "video")) return;
    return navigate({ kind: "create", projectId: route.projectId, stage: "video", shotId: input.shotId });
  };
  const reuse = (input: RunInput) => mutate(async () => {
    if (!input.selectionRef || detail?.projectId !== route.projectId || detail.ref.id !== route.run?.id || detail.ref.source !== route.run?.source) return;
    // Video-only product: reuse never opens the legacy image/studio stage.
    const video = await productClient.creation.generatorsList(route.projectId, "video");
    if (!mounted.current || scope !== liveScope.current) return;
    const option = video.find(option => option.selectionRef === input.selectionRef);
    if (!option || option.mediaKind !== "video" || !option.availability) throw { code: "GENERATOR_UNAVAILABLE" };
    const sources = detail?.detail?.sources.filter(source => source.stage === "video") ?? [];
    // New readers provide a formal per-input locator. Explicit null means
    // unresolved; only older readers without this field retain the single-source fallback.
    if (input.shotId && !sources.some(source => source.id === input.shotId)) throw { code: "PROJECT_SCOPE_VIOLATION" };
    const shotId = input.shotId !== undefined ? input.shotId : sources.length === 1 ? sources[0].id : null;
    const requestId = crypto.randomUUID();
    const store = useStudioStore.getState();
    store.setPendingRunIntent({ projectId: route.projectId, stage: "video", selectionRef: input.selectionRef, values: input.values,
      target: { shotId, requestId, approved: false } });
    let approved = false;
    try {
      approved = await navigate({ kind: "create", projectId: route.projectId, stage: "video", shotId: shotId ?? undefined }) !== false;
    } finally {
      // Never clear/approve a newer operation, including a different Run chosen
      // while a navigation confirmation was pending.
      const pending = useStudioStore.getState().pendingRunIntent;
      if (pending?.target?.requestId === requestId) {
        useStudioStore.getState().setPendingRunIntent(approved ? { ...pending, target: { ...pending.target, approved: true } } : undefined);
      }
    }
  });
  const selectForShot = (shotId: string, assetId: string) => mutate(async () => {
    if (!detail || detail.projectId !== route.projectId || detail.ref.id !== route.run?.id || detail.ref.source !== route.run?.source) return;
    if (!detail.detail?.sources.some(source => source.id === shotId && source.stage === "video")) return;
    const target = results.find(r => r.assetId === assetId);
    if (!target || !isSelectableShotResult(target, shotId)) return;
    await productClient.creation.selectResult(route.projectId, shotId, "video", assetId);
  });
  const approveAndSelectForShot = async (result: RunResult, shotId: string, comment = "") => {
    if (!detail || result.reviewRevision === null || result.reviewState !== "PENDING" || !isSelectableShotResult(result, shotId)) return false;
    if (detail.projectId !== route.projectId || detail.ref.id !== route.run?.id || detail.ref.source !== route.run?.source
      || !detail.detail?.sources.some(source => source.id === shotId && source.stage === "video")) return false;
    const expectedRevision = result.reviewRevision;
    let reviewed = false;
    return mutate(async () => {
      try {
        await productClient.run.resultReview(route.projectId, { runRef: detail.ref, assetId: result.assetId, decision: "APPROVED", comment, expectedRevision });
        reviewed = true;
        await productClient.creation.selectResult(route.projectId, shotId, "video", result.assetId);
      } catch (cause) {
        if (reviewed) invalidateRuns(route.projectId);
        throw cause;
      }
    });
  };
  const review = async (result: RunResult, decision: "APPROVED" | "REJECTED", comment = "") => {
    if (!detail || result.reviewRevision === null) return false;
    const expectedRevision=result.reviewRevision;
    return mutate(async () => {
    await productClient.run.resultReview(route.projectId, { runRef: detail.ref, assetId: result.assetId, decision, comment, expectedRevision });
    });
  };
  const reopenReview = (result: RunResult) => mutate(async () => {
    if (!detail || result.reviewRevision === null || !window.confirm("重新打开审核？原审核记录保留。")) return;
    await productClient.run.resultReviewReset(route.projectId, detail.ref, result.assetId, result.reviewRevision);
  });
  return { reopenReview, list, detail, results, error: actionError ?? error, missing, loading, busy, filter, action, retryAndStart, remove, retryInput, retryInputAndStart, openShot, reuse, review, selectForShot, approveAndSelectForShot, refresh };
}
export type RunsController = ReturnType<typeof useRunsController>;

function isSelectableShotResult(result: RunResult, shotId: string) {
  return result.sourceShotIds?.length === 1 && result.sourceShotIds[0] === shotId
    && result.mediaKind === "video" && result.assetExists && result.availability !== "missing"
    && result.availability !== "unavailable" && result.reviewState !== "REJECTED";
}
