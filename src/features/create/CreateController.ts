import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { CreationAccepted, CreationContext, CreationReadiness, GeneratorOption, ProductRun, RunRef } from "../../product/types";
import type { GenerationValues, DraftValue } from "../../types/generation";
import type { AppRoute } from "../../app/routes/types";
import { useStudioStore } from "../../stores/studioStore";
import { draftFor, mediaKind, mediaValue, scopeKey, type CreateRoute } from "./createModel";
export interface CreateProps { route: CreateRoute; navigate: (route: AppRoute) => unknown; onDirtyChange?: (dirty: boolean) => void }
interface StageView { selection: string; values: GenerationValues; dirty: boolean; runRef: RunRef | null; accepted: CreationAccepted | null }
export function useCreateController({ route, navigate, onDirtyChange }: CreateProps) {
  const values = useStudioStore(state => state.values);
  const dirty = useStudioStore(state => state.draftDirty);
  const [context, setContext] = useState<CreationContext>();
  const [generators, setGenerators] = useState<GeneratorOption[]>([]);
  const [selection, setSelection] = useState("");
  const [readiness, setReadiness] = useState<CreationReadiness>();
  const [accepted, setAccepted] = useState<CreationAccepted | null>(null);
  const [runRef, setRunRef] = useState<RunRef | null>(null);
  const [run, setRun] = useState<ProductRun>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const snapshots = useRef(new Map<string, StageView>());
  const live = useRef({ selection, runRef, accepted }); live.current = { selection, runRef, accepted };
  const epoch = useRef(0);
  const attempt = useRef<string | null>(null);
  const actionLock = useRef(false);
  const owner = `${route.projectId}:${route.shotId ?? ""}`;
  const previousOwner = useRef(owner);
  const key = scopeKey(route);
  const generator = generators.find(item => item.selectionRef === selection);
  useEffect(() => {
    if (previousOwner.current !== owner) { snapshots.current.clear(); previousOwner.current = owner; }
    const token = ++epoch.current;
    let cancelled = false; let initialized = false;
    setLoading(true); setContext(undefined); setReadiness(undefined); setRun(undefined); setError(undefined);
    attempt.current = null;
    const store = useStudioStore.getState();
    store.loadCreationDraft({});
    Promise.all([productClient.creation.get(route.projectId, route.shotId ?? null, route.stage), productClient.creation.generatorsList(route.projectId, route.stage, route.shotId ?? null)])
      .then(([next, options]) => {
        if (cancelled || token !== epoch.current) return;
        const saved = snapshots.current.get(key);
        const selected = options.find(item => item.selectionRef === saved?.selection) ?? options.find(item => item.selectionRef === next.selectedShot?.selectionRef) ?? options.find(item => item.recommended && item.availability) ?? options.find(item => item.availability);
        initialized = true; setContext(next); setGenerators(options); setSelection(selected?.selectionRef ?? "");
        useStudioStore.getState().loadCreationDraft(saved?.values ?? (selected ? draftFor(selected, next.selectedShot) : {}), saved?.dirty ?? false);
        setRunRef(saved?.runRef ?? next.selectedShot?.recentRun ?? null); setAccepted(saved?.accepted ?? null);
      }).catch(error => { if (!cancelled) setError(normalizeProductError(error).message); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => {
      cancelled = true;
      const store = useStudioStore.getState();
      if (initialized) snapshots.current.set(key, { ...live.current, values: store.values, dirty: store.draftDirty });
    };
  }, [key]); // Canonical route is the only Shot/stage owner.
  useEffect(() => { onDirtyChange?.(dirty || [...snapshots.current.entries()].some(([scope, item]) => scope !== key && item.dirty)); }, [dirty, key, loading, onDirtyChange]);
  useEffect(() => () => { onDirtyChange?.(false); useStudioStore.getState().loadCreationDraft({}); }, [onDirtyChange]);
  function submission() {
    return { projectId: route.projectId, shotId: route.shotId ?? "", stage: route.stage, selectionRef: selection, values: useStudioStore.getState().values, submissionIdempotencyKey: attempt.current ?? "readiness-only" };
  }
  useEffect(() => {
    setReadiness(undefined);
    if (loading || !route.shotId || !generator?.availability) return;
    let cancelled = false;
    const timer = setTimeout(() => { productClient.creation.readinessGet(submission()).then(value => { if (!cancelled) setReadiness(value); }).catch(error => { if (!cancelled) setError(normalizeProductError(error).message); }); }, 600);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [values, selection, loading, key]);
  async function refreshContext(token = epoch.current) {
    const next = await productClient.creation.get(route.projectId, route.shotId ?? null, route.stage);
    if (token === epoch.current) setContext(next);
  }
  useEffect(() => {
    if (loading || !runRef) return;
    let cancelled = false; let timer: ReturnType<typeof setTimeout>;
    const token = epoch.current;
    async function poll() {
      try {
        const next = await productClient.run.get(route.projectId, runRef!);
        if (cancelled) return;
        setRun(next);
        if (["SUCCEEDED", "PARTIAL", "FAILED", "CANCELLED"].includes(next.status)) { await refreshContext(token); return; }
      } catch (error) { if (!cancelled) setError(normalizeProductError(error).message); }
      if (!cancelled) timer = setTimeout(poll, 1500);
    }
    void poll(); return () => { cancelled = true; clearTimeout(timer); };
  }, [runRef, key, loading]);
  // Existing Library intent is consumed into a schema-selected draft slot only.
  const intent = useStudioStore(state => state.pendingAssetIntent);
  useEffect(() => {
    if (loading || !route.shotId || !context?.selectedShot || !generator || !intent || intent.projectId !== route.projectId) return;
    const asset = context.mediaInputs.find(item => item.id === intent.assetId && item.mediaKind === intent.assetType);
    if (!asset) { setError("此素材不存在或不属于当前项目。"); useStudioStore.getState().clearPendingAssetIntent(); return; }
    const fields = generator.fields.filter(field => mediaKind(field) === asset.mediaKind);
    if (fields.length === 1) { setValue(fields[0].key, mediaValue(fields[0], [asset.id])); useStudioStore.getState().clearPendingAssetIntent(); }
    else setError("请在明确的首帧、尾帧或素材输入框中选择此素材。");
  }, [intent, generator, loading, context, key]);
  function setValue(field: string, value: DraftValue) { const intent = useStudioStore.getState().pendingAssetIntent; if (intent && (("assetId" in value && value.assetId === intent.assetId) || ("assetIds" in value && value.assetIds.includes(intent.assetId)))) useStudioStore.getState().clearPendingAssetIntent(); attempt.current = null; setAccepted(null); useStudioStore.getState().setValue(field, value); }
  function removeValue(field: string) { attempt.current = null; useStudioStore.getState().removeValue(field); }
  function chooseGenerator(ref: string) {
    const option = generators.find(item => item.selectionRef === ref); if (!option) return;
    attempt.current = null; setSelection(ref); setAccepted(null);
    useStudioStore.getState().loadCreationDraft(draftFor(option, context?.selectedShot ?? null, values), true);
  }
  async function mutate(action: () => Promise<void>) {
    if (actionLock.current) return;
    actionLock.current = true; setBusy(true); setError(undefined);
    try { await action(); } catch (error) { setError(normalizeProductError(error).message); }
    finally { actionLock.current = false; setBusy(false); }
  }
  function generate() { return mutate(async () => {
    const token = epoch.current;
    attempt.current ??= crypto.randomUUID();
    const request = submission();
    const ready = await productClient.creation.readinessGet(request);
    if (token !== epoch.current) return;
    setReadiness(ready); if (!ready.ready) return;
    const ack = await productClient.creation.generate(request);
    if (token !== epoch.current) return;
    setAccepted(ack); setRunRef(ack.runRef); setRun(undefined);
    useStudioStore.getState().loadCreationDraft(request.values, false);
  }); }
  const createShot = () => mutate(async () => { const token = epoch.current; const shot = await productClient.creation.createShot(route.projectId); if (token === epoch.current) navigate({ ...route, shotId: shot.id }); });
  const selectResult = (id: string) => mutate(async () => { const token = epoch.current; await productClient.creation.selectResult(route.projectId, route.shotId!, route.stage, id); await refreshContext(token); });
  const setReferences = (ids: string[]) => mutate(async () => { const token = epoch.current; await productClient.creation.referencesSet(route.projectId, route.shotId!, route.stage, ids); await refreshContext(token); });
  const retry = () => mutate(async () => { if (!run?.availableActions.includes("RETRY")) return; const next = await productClient.run.retry(route.projectId, { ref: run.ref, selectedItemIds: run.recoverability.retryItemIds }); setRun(next); setRunRef(next.ref); setAccepted(null); });
  return { context, generators, generator, selection, values, readiness, accepted, run, runRef, error, loading, busy,
    setValue, removeValue, chooseGenerator, generate, createShot, selectResult, setReferences, retry,
    refresh: () => mutate(() => refreshContext()),
    selectShot: (shotId: string) => navigate({ ...route, shotId }),
    selectStage: (stage: "image" | "video") => navigate({ ...route, stage }),
    openRun: () => runRef && navigate({ kind: "runs", projectId: route.projectId, run: runRef }),
    openAdvanced: (section: "batch" | "production" | "workflows") => navigate(section === "batch" ? { ...route, surface: "batch" } : section === "production" ? { kind: "runs", projectId: route.projectId, filter: "production" } : { kind: "project-settings", projectId: route.projectId, section: "advanced-workflows" }),
  };
}
export type CreateController = ReturnType<typeof useCreateController>;
