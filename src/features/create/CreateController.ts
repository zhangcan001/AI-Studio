import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { CreationAccepted, CreationContext, CreationReadiness, GeneratorOption, ProductRun, RunRef, CreationPromptChoice, CreationPromptProvenance } from "../../product/types";
import type { GenerationValues, DraftValue } from "../../types/generation";
import type { AppRoute } from "../../app/routes/types";
import { useStudioStore } from "../../stores/studioStore";
import type { ComfyStatus } from "../../types/comfy";
import { assetIds, draftFor, mediaKind, mediaValue, scopeKey, type CreateRoute } from "./createModel";
export interface CreateProps { route: CreateRoute; runtime?: ComfyStatus; navigate: (route: AppRoute) => unknown; onDirtyChange?: (dirty: boolean) => void }
interface StageView { selection: string; values: GenerationValues; dirty: boolean; runRef: RunRef | null; accepted: CreationAccepted | null; provenance?: CreationPromptProvenance }
export function useCreateController({ route, runtime, navigate, onDirtyChange }: CreateProps) {
  const values = useStudioStore(state => state.values);
  const dirty = useStudioStore(state => state.draftDirty);
  const provenance = useStudioStore(state => state.creationPromptProvenance);
  const [context, setContext] = useState<CreationContext>();
  const [generators, setGenerators] = useState<GeneratorOption[]>([]);
  const [selection, setSelection] = useState("");
  const [accepted, setAccepted] = useState<CreationAccepted | null>(null);
  const [runRef, setRunRef] = useState<RunRef | null>(null);
  const [run, setRun] = useState<ProductRun>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const snapshots = useRef(new Map<string, StageView>());
  const live = useRef({ selection, runRef, accepted }); live.current = { selection, runRef, accepted };
  const epoch = useRef(0);
  const readinessEpoch = useRef(0);
  const mounted = useRef(false);
  const attempt = useRef<string | null>(null);
  const actionLock = useRef(false);
  const owner = `${route.projectId}:${route.shotId ?? ""}`;
  const previousOwner = useRef(owner);
  const key = scopeKey(route);
  const runtimeStatus = runtime?.status, runtimeEndpoint = runtime?.endpoint, runtimeGeneration = runtime?.runtimeGeneration;
  const currentDraft = useRef({ key, selection, values, provenance, runtimeStatus, runtimeEndpoint, runtimeGeneration });
  currentDraft.current = { key, selection, values, provenance, runtimeStatus, runtimeEndpoint, runtimeGeneration };
  const [readinessCheck, setReadinessCheck] = useState<{ result: CreationReadiness; draft: typeof currentDraft.current }>();
  function matchesDraft(draft: typeof currentDraft.current) {
    const current = currentDraft.current;
    return draft.key === current.key && draft.selection === current.selection && draft.values === current.values && draft.provenance === current.provenance
      && draft.runtimeStatus === current.runtimeStatus && draft.runtimeEndpoint === current.runtimeEndpoint && draft.runtimeGeneration === current.runtimeGeneration;
  }
  // Reject a stale positive on the render itself, before effect cleanup runs.
  const readiness = readinessCheck && matchesDraft(readinessCheck.draft) && runtimeStatus === "CONNECTED" ? readinessCheck.result : undefined;
  const readinessState = !runtime ? "CHECKING" : runtimeStatus !== "CONNECTED" ? "RUNTIME_OFFLINE" : readiness ? readiness.ready ? "READY" : "NOT_READY" : "CHECKING";
  const generator = generators.find(item => item.selectionRef === selection);
  useEffect(() => {
    if (previousOwner.current !== owner) { snapshots.current.clear(); previousOwner.current = owner; }
    const token = ++epoch.current;
    let cancelled = false; let initialized = false;
    setLoading(true); setContext(undefined); setReadinessCheck(undefined); setRun(undefined); setError(undefined);
    attempt.current = null;
    const store = useStudioStore.getState();
    const labReturn = store.creationLabReturn?.scope === key ? { ...store.creationLabReturn, values: store.values, dirty: store.draftDirty, provenance: store.creationPromptProvenance } : undefined;
    if (!labReturn) store.loadCreationDraft({});
    Promise.all([productClient.creation.get(route.projectId, route.shotId ?? null, route.stage), productClient.creation.generatorsList(route.projectId, route.stage, route.shotId ?? null)])
      .then(([next, options]) => {
        if (cancelled || token !== epoch.current) return;
        const saved = labReturn ? { selection: labReturn.selectionRef, values: labReturn.values, dirty: labReturn.dirty, runRef: labReturn.runRef, accepted: labReturn.accepted, provenance: labReturn.provenance } : snapshots.current.get(key);
        const intent = useStudioStore.getState().pendingRunIntent;
        const reuse = intent?.projectId === route.projectId && intent.stage === route.stage ? intent : undefined;
        const selected = reuse ? options.find(item => item.selectionRef === reuse.selectionRef) : saved ? options.find(item => item.selectionRef === saved.selection) : options.find(item => item.selectionRef === next.selectedShot?.selectionRef) ?? options.find(item => item.recommended && item.availability) ?? options.find(item => item.availability);
        if ((reuse || saved) && !selected) setError("原生成器当前不可用，请明确选择其他生成器；未自动替换。");
        initialized = true; setContext(next); setGenerators(options); setSelection(selected?.selectionRef ?? "");
        const reused = reuse && selected?.selectionRef === reuse.selectionRef && next.selectedShot;
        useStudioStore.getState().loadCreationDraft(reused ? draftFor(selected, next.selectedShot, reuse.values) : saved?.values ?? (selected ? draftFor(selected, next.selectedShot) : {}), reused ? true : saved?.dirty ?? false, reused ? undefined : saved?.provenance);
        if (reused) useStudioStore.getState().setPendingRunIntent(undefined);
        setRunRef(saved?.runRef ?? next.selectedShot?.recentRun ?? null); setAccepted(saved?.accepted ?? null);
        useStudioStore.getState().setCreationLabReturn(undefined);
      }).catch(error => { if (!cancelled) setError(normalizeProductError(error).message); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => {
      cancelled = true;
      const store = useStudioStore.getState();
      if (initialized) snapshots.current.set(key, { ...live.current, values: store.values, dirty: store.draftDirty, provenance: store.creationPromptProvenance });
    };
  }, [key]); // Canonical route is the only Shot/stage owner.
  useEffect(() => { onDirtyChange?.(dirty || [...snapshots.current.entries()].some(([scope, item]) => scope !== key && item.dirty)); }, [dirty, key, loading, onDirtyChange]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => () => { onDirtyChange?.(false); const store = useStudioStore.getState(); if (!store.creationLabReturn) store.loadCreationDraft({}); }, [onDirtyChange]);
  function submission() {
    const store = useStudioStore.getState();
    return { projectId: route.projectId, shotId: route.shotId ?? "", stage: route.stage, selectionRef: selection, values: store.values, ...store.creationPromptProvenance, submissionIdempotencyKey: attempt.current ?? "readiness-only" };
  }
  useEffect(() => {
    ++readinessEpoch.current;
    setReadinessCheck(undefined);
  }, [values, provenance, selection, loading, key, runtimeStatus, runtimeEndpoint, runtimeGeneration]);
  useEffect(() => {
    if (loading || busy || readiness || runtimeStatus !== "CONNECTED" || !route.shotId || !generator?.availability) return;
    const timer = setTimeout(() => { void recheckReadiness(); }, 600);
    return () => { ++readinessEpoch.current; clearTimeout(timer); };
  }, [values, provenance, selection, loading, key, runtimeStatus, runtimeEndpoint, runtimeGeneration, busy]);
  async function recheckReadiness() {
    if (loading || !route.shotId || !generator?.availability || actionLock.current) return;
    const token = epoch.current;
    const requestEpoch = ++readinessEpoch.current;
    const draft = currentDraft.current;
    const isCurrent = () => mounted.current && token === epoch.current && requestEpoch === readinessEpoch.current && matchesDraft(draft);
    try {
      const next = await productClient.creation.readinessGet({ ...submission(), submissionIdempotencyKey: "readiness-only" });
      if (isCurrent()) { setReadinessCheck({ result: next, draft }); setError(undefined); }
    } catch (error) { if (isCurrent()) setError(normalizeProductError(error).message); }
  }
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
  const libraryIntent = useStudioStore(state => state.pendingLibraryIntent);
  useEffect(() => {
    if (!libraryIntent) return;
    if (libraryIntent.projectId !== route.projectId) { useStudioStore.getState().setPendingLibraryIntent(undefined); return; }
    if (loading || !context) return;
    if (libraryIntent.kind === "prompt" && context.selectedShot && generator?.fields.some(field => field.key === "prompt" && field.type === "textarea")) {
      applyPromptChoice(libraryIntent);
      useStudioStore.getState().setPendingLibraryIntent(undefined);
    }
    if (libraryIntent.kind !== "asset" || context.mediaInputs.some(a => a.id === libraryIntent.assetId)) return;
    let alive=true; const token=epoch.current; const project=route.projectId;
    void productClient.library.get(route.projectId,{kind:"asset",id:libraryIntent.assetId}).then(detail=>{
      if(!alive || token!==epoch.current || detail.kind!=="asset" || detail.asset.assetType!==libraryIntent.mediaKind)return;
      setContext(current=>current?.projectId===project ? {...current,mediaInputs:[...current.mediaInputs.filter(a=>a.id!==detail.asset.id),{id:detail.asset.id,name:detail.asset.name,mediaKind:libraryIntent.mediaKind,selected:false}]} : current);
    }).catch(error=>{if(alive)setError(normalizeProductError(error).message);});
    return()=>{alive=false;};
  },[libraryIntent,loading,context,generator,key]);
  // A returned draft may still reference an older asset after its intent was consumed.
  // Resolve only those explicit field identities, never widen the recent picker.
  useEffect(() => {
    if (loading || !context || !generator) return;
    const missing=generator.fields.flatMap(field=>assetIds(values[field.key]).map(id=>({id,kind:mediaKind(field)})))
      .filter(item=>item.kind && !context.mediaInputs.some(a=>a.id===item.id));
    if (!missing.length) return;
    let alive=true; const token=epoch.current; const project=route.projectId;
    void Promise.all([...new Map(missing.map(item=>[item.id,item])).values()].map(async item=>{
      const detail=await productClient.library.get(project,{kind:"asset",id:item.id});
      return detail.kind==="asset" && detail.asset.assetType===item.kind ? {id:item.id,name:detail.asset.name,mediaKind:item.kind!,selected:false} : undefined;
    })).then(items=>{
      if (!alive || token!==epoch.current) return;
      if (!items.some(Boolean)) {setError("草稿素材不存在或类型不兼容，请重新选择。");return;}
      setContext(current=>current?.projectId===project ? {...current,mediaInputs:[...current.mediaInputs,...items.filter((a):a is NonNullable<typeof a>=>!!a && !current.mediaInputs.some(b=>b.id===a.id))]} : current);
    }).catch(error=>{if(alive && token===epoch.current)setError(normalizeProductError(error).message);});
    return()=>{alive=false;};
  },[values,loading,context,generator,key]);
  const librarySlots = libraryIntent?.kind === "asset" ? generator?.fields.filter(field=>mediaKind(field)===libraryIntent.mediaKind) ?? [] : [];
  function applyLibraryAsset(fieldKey:string) {
    if(libraryIntent?.kind!=="asset" || libraryIntent.projectId!==route.projectId || !context?.selectedShot)return;
    const field=librarySlots.find(f=>f.key===fieldKey);
    if(!field || !context.mediaInputs.some(a=>a.id===libraryIntent.assetId && a.mediaKind===libraryIntent.mediaKind))return;
    setValue(field.key,mediaValue(field,[libraryIntent.assetId]));
    useStudioStore.getState().setPendingLibraryIntent(undefined);
  }
  function setValue(field: string, value: DraftValue) { const intent = useStudioStore.getState().pendingAssetIntent; if (intent && (("assetId" in value && value.assetId === intent.assetId) || ("assetIds" in value && value.assetIds.includes(intent.assetId)))) useStudioStore.getState().clearPendingAssetIntent(); attempt.current = null; setAccepted(null); useStudioStore.getState().setValue(field, value); }
  function removeValue(field: string) { attempt.current = null; setAccepted(null); useStudioStore.getState().removeValue(field); }
  function chooseResolution(id: string) {
    const preset = generator?.resolutionPresets?.find(item => item.id === id);
    if (!preset || busy) return;
    attempt.current = null; setAccepted(null);
    const store = useStudioStore.getState();
    store.loadCreationDraft({ ...store.values, width: { type: "integer", value: preset.width }, height: { type: "integer", value: preset.height } }, true, store.creationPromptProvenance);
  }
  function applyPromptChoice(choice: CreationPromptChoice | (CreationPromptProvenance & {text:string})) {
    if (!context?.selectedShot || !generator?.fields.some(field=>field.key==="prompt" && field.type==="textarea")) return;
    attempt.current=null; setAccepted(null); useStudioStore.getState().applyCreationPrompt(choice);
  }
  function openLibrary(filter: "prompts" | "images" | "videos") {
    useStudioStore.getState().setCreationLabReturn({scope:key, route, selectionRef:selection, runRef, accepted});
    return navigate({kind:"library",projectId:route.projectId,filter});
  }
  function chooseGenerator(ref: string) {
    const option = generators.find(item => item.selectionRef === ref); if (!option) return;
    useStudioStore.getState().setPendingRunIntent(undefined);
    attempt.current = null; setSelection(ref); setAccepted(null);
    useStudioStore.getState().loadCreationDraft(draftFor(option, context?.selectedShot ?? null, values), true, option.fields.some(field=>field.key==="prompt" && field.type==="textarea") ? useStudioStore.getState().creationPromptProvenance : undefined);
  }
  async function mutate(action: () => Promise<void>) {
    if (actionLock.current) return;
    actionLock.current = true; setBusy(true); setError(undefined);
    try { await action(); } catch (error) { setError(normalizeProductError(error).message); }
    finally { actionLock.current = false; setBusy(false); }
  }
  function generate() { return mutate(async () => {
    const token = epoch.current;
    const draft = currentDraft.current;
    ++readinessEpoch.current; // An earlier read-only check cannot replace submit-time readiness.
    attempt.current ??= crypto.randomUUID();
    const request = submission();
    const ready = await productClient.creation.readinessGet(request);
    if (!mounted.current || token !== epoch.current || !matchesDraft(draft)) return;
    setReadinessCheck({ result: ready, draft }); if (!ready.ready || draft.runtimeStatus !== "CONNECTED") return;
    const ack = await productClient.creation.generate(request);
    if (token !== epoch.current) return;
    setAccepted(ack); setRunRef(ack.runRef); setRun(undefined);
    useStudioStore.getState().loadCreationDraft(request.values, false, request.promptId && request.promptVersionId ? {promptId:request.promptId,promptVersionId:request.promptVersionId} : undefined);
  }); }
  const createShot = () => mutate(async () => { const token = epoch.current; const shot = await productClient.creation.createShot(route.projectId); if (token === epoch.current) navigate({ ...route, shotId: shot.id }); });
  const selectResult = (id: string) => mutate(async () => { const token = epoch.current; await productClient.creation.selectResult(route.projectId, route.shotId!, route.stage, id); await refreshContext(token); });
  const setReferences = (ids: string[]) => mutate(async () => { const token = epoch.current; await productClient.creation.referencesSet(route.projectId, route.shotId!, route.stage, ids); await refreshContext(token); });
  const retry = () => mutate(async () => { if (!run?.availableActions.includes("RETRY")) return; const next = await productClient.run.retry(route.projectId, { ref: run.ref, selectedItemIds: run.recoverability.retryItemIds }); setRun(next); setRunRef(next.ref); setAccepted(null); });
  return { libraryIntent, librarySlots, applyLibraryAsset, context, generators, generator, selection, values, readiness, readinessState, accepted, run, runRef, error, loading, busy,
    setValue, removeValue, chooseResolution, applyPromptChoice, openLibrary, chooseGenerator, generate, createShot, selectResult, setReferences, retry, recheckReadiness,
    openRuntimeSettings: () => {
      useStudioStore.getState().setCreationLabReturn({ scope: key, selectionRef: selection, runRef, accepted });
      return navigate({ kind: "system-settings", section: "general", returnTo: route });
    },
    openProjects: () => navigate({ kind: "project-list" }),
    refresh: () => mutate(() => refreshContext()),
    selectShot: (shotId: string) => navigate({ ...route, shotId }),
    selectStage: (stage: "image" | "video") => navigate({ ...route, stage }),
    openRun: () => runRef && navigate({ kind: "runs", projectId: route.projectId, run: runRef }),
    openAdvanced: (section: "batch" | "production" | "workflows") => {
      if (section === "workflows") useStudioStore.getState().setCreationLabReturn({ scope: key, selectionRef: selection, runRef, accepted });
      return navigate(section === "batch" ? { ...route, surface: "batch" } : section === "production" ? { kind: "runs", projectId: route.projectId, filter: "production" } : { kind: "system-settings", section: "advanced-workflows", returnTo: route });
    },
  };
}
export type CreateController = ReturnType<typeof useCreateController>;
