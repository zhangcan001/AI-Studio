import { useCallback, useEffect, useRef, useState } from "react";
import type { AppRoute } from "../../app/routes/types";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns, subscribeRunInvalidation } from "../../product/runInvalidation";
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
      // Product list is a recent projection only; never request or invent a mixed-history cursor.
      const page = await productClient.run.list(route.projectId, filter, null);
      if (token !== epoch.current) return;
      setList(page); // Keep coverage/nextCursor evidence; UI never paginates mixed history.
      if (route.run) {
        try {
          const run = await productClient.run.get(route.projectId, route.run);
          const outputs = await productClient.run.resultsGet(route.projectId, route.run);
          if (token !== epoch.current) return;
          setDetail(run); setResults(outputs); setMissing(false);
        } catch (error) {
          if (token !== epoch.current) return;
          const issue = normalizeProductError(error);
          if (issue.code !== "RUN_NOT_FOUND" && issue.code !== "PROJECT_SCOPE_VIOLATION") throw issue;
          setMissing(true); setDetail(undefined); setResults([]);
        }
      }
      if (token === epoch.current) setError(undefined);
    } catch (error) { if (token === epoch.current) setError(normalizeProductError(error).message); }
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
    const timer = setInterval(() => void refresh(), 5000);
    return () => { ++epoch.current; unsubscribe(); clearTimeout(notification); clearInterval(timer); };
  }, [refresh]);
  async function mutate(action: () => Promise<unknown>) {
    if (actionLock.current) return;
    actionLock.current = true; setBusy(true); setActionError(undefined); const owner = scope;
    try { await action(); invalidateRuns(route.projectId); }
    catch (error) { if (owner === liveScope.current) setActionError(normalizeProductError(error).message); }
    finally { actionLock.current = false; setBusy(false); }
  }
  const action = (kind: "START" | "PAUSE" | "CANCEL" | "RETRY") => mutate(async () => {
    if (!detail?.availableActions.includes(kind)) return;
    if (kind === "RETRY") await productClient.run.retry(route.projectId, { ref: detail.ref, selectedItemIds: detail.recoverability.retryItemIds });
    else await productClient.run[kind === "START" ? "start" : kind === "PAUSE" ? "pause" : "cancel"](route.projectId, detail.ref);
  });
  const reuse = (input: RunInput) => mutate(async () => {
    if (!input.selectionRef) return;
    // Video-only product: reuse never opens the legacy image/studio stage.
    const video = await productClient.creation.generatorsList(route.projectId, "video");
    if (!mounted.current || scope !== liveScope.current) return;
    const option = video.find(option => option.selectionRef === input.selectionRef);
    if (!option || option.mediaKind !== "video" || !option.availability) throw { code: "GENERATOR_UNAVAILABLE" };
    const sources = detail?.detail?.sources.filter(source => source.stage === "video") ?? [];
    // RunInput has no per-input Shot locator. Mixed batches must ask for a
    // target instead of guessing that every input belongs to the first source.
    const shotId = sources.length === 1 ? sources[0].id : null;
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
  const review = (result: RunResult, decision: "APPROVED" | "REJECTED", comment = "") => mutate(async () => {
    if (!detail || result.reviewRevision === null) return;
    await productClient.run.resultReview(route.projectId, { runRef: detail.ref, assetId: result.assetId, decision, comment, expectedRevision: result.reviewRevision });
  });
  return { list, detail, results, error: actionError ?? error, missing, loading, busy, filter, action, reuse, review, refresh };
}
export type RunsController = ReturnType<typeof useRunsController>;
