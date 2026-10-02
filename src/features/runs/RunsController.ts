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
  const filter = filterFor(route);
  const scope = `${route.projectId}:${route.run?.source ?? ""}:${route.run?.id ?? ""}:${filter}`;
  const liveScope = useRef(scope); liveScope.current = scope;
  const refresh = useCallback(async () => {
    const token = ++epoch.current;
    try {
      const page = await productClient.run.list(route.projectId, filter);
      if (token !== epoch.current) return;
      setList(page);
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
    finally { if (token === epoch.current) setLoading(false); }
  }, [route.projectId, route.run?.source, route.run?.id, filter]);
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
    const [image, video] = await Promise.all([productClient.creation.generatorsList(route.projectId, "image"), productClient.creation.generatorsList(route.projectId, "video")]);
    if (scope !== liveScope.current) return;
    const option = [...image, ...video].find(option => option.selectionRef === input.selectionRef);
    if (!option) throw { code: "GENERATOR_UNAVAILABLE" };
    useStudioStore.getState().setPendingRunIntent({ projectId: route.projectId, stage: option.mediaKind, selectionRef: input.selectionRef, values: input.values });
    navigate({ kind: "create", projectId: route.projectId, stage: option.mediaKind, shotId: detail?.detail?.sources.find(source => source.stage === option.mediaKind)?.id });
  });
  const review = (result: RunResult, decision: "APPROVED" | "REJECTED", comment = "") => mutate(async () => {
    if (!detail || result.reviewRevision === null) return;
    await productClient.run.resultReview(route.projectId, { runRef: detail.ref, assetId: result.assetId, decision, comment, expectedRevision: result.reviewRevision });
  });
  return { list, detail, results, error: actionError ?? error, missing, loading, busy, filter, action, reuse, review, refresh };
}
export type RunsController = ReturnType<typeof useRunsController>;
