import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateVideoInputs, subscribeVideoInputInvalidation } from "../../product/videoInputInvalidation";
import type { GeneratorOption } from "../../product/types";
import type { VideoInputAsset, VideoInputKey, VideoInputView } from "../../types/shotVideoInput";
import { useStudioStore } from "../../stores/studioStore";
import { assetIds, mediaKind, mediaValue, type CreateRoute } from "./createModel";
import type { GenerationValues } from "../../types/generation";
import type { RecipeField } from "../../types/generation";

function pluralField(field: RecipeField | undefined) {
  return !!field && (field.type === "images" || field.type === "videos" || field.type === "audios");
}

export function usePersistentVideoInputs(route: CreateRoute, generator: GeneratorOption | undefined, pageLoading: boolean, restoredDraft?: GenerationValues) {
  const enabled = route.stage === "video" && !!generator?.persistentInputs && !!route.shotId;
  const owner = JSON.stringify([route.projectId, route.shotId, generator?.selectionRef]);
  const currentOwner = useRef(owner); currentOwner.current = owner;
  const epoch = useRef(0); const lock = useRef(false); const selfWrite = useRef(false);
  const readEpoch = useRef(0), pendingRefresh = useRef(false);
  const values = useStudioStore(state => state.values);
  const [state, setState] = useState<{ owner: string; view: VideoInputView }>();
  const liveState = useRef(state); liveState.current = state;
  const [loading, setLoading] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState<string>();
  const [importReceipt, setImportReceipt] = useState<string>();
  const selection = { projectId: route.projectId, shotId: route.shotId ?? "", selectionRef: generator?.selectionRef ?? "" };
  const view = state?.owner === owner ? state.view : undefined;
  const fields = generator?.fields.filter(field => mediaKind(field)) ?? [];
  const inputs: VideoInputAsset[] = fields.flatMap(field => assetIds(values[field.key]).map((assetId, ordinal) => ({ inputKey: field.key as VideoInputKey, ordinal, assetId })));
  const ordered = (bindings: VideoInputAsset[]) => JSON.stringify([...bindings].sort((a,b) => a.inputKey.localeCompare(b.inputKey) || a.ordinal-b.ordinal));
  const dirty = enabled && !!view && ordered(inputs) !== ordered(view.inputs);
  const currentBindings = () => fields.flatMap(field => assetIds(useStudioStore.getState().values[field.key])
    .map((assetId, ordinal) => ({ inputKey: field.key as VideoInputKey, ordinal, assetId })));
  function acceptView(own: string, next: VideoInputView) {
    const accepted = { owner: own, view: next };
    liveState.current = accepted; setState(accepted);
  }

  function draftFromBindings(bindings: VideoInputAsset[]) {
    const store = useStudioStore.getState();
    const merged = Object.fromEntries(Object.entries(store.values).filter(([,v]) => !("assetId" in v || "assetIds" in v)));
    for (const field of fields) {
      const ids = bindings.filter(i => i.inputKey === field.key).sort((a,b)=>a.ordinal-b.ordinal).map(i=>i.assetId);
      if (ids.length) merged[field.key] = mediaValue(field, ids);
    }
    store.loadCreationDraft(merged, store.draftDirty, store.creationPromptProvenance);
  }

  async function load(restore = false, background = false) {
    if (!enabled || pageLoading) return;
    if (lock.current) { pendingRefresh.current = true; return; }
    const token = epoch.current, read = ++readEpoch.current; const own = owner;
    setLoading(true); if (!background) setError(undefined);
    try {
      const next = await productClient.creation.inputsGet(selection);
      if (own !== currentOwner.current || token !== epoch.current || read !== readEpoch.current) return;
      const store = useStudioStore.getState();
      const previous = liveState.current?.owner === own ? liveState.current.view : undefined;
      // A delayed same-instance read cannot roll back acknowledged OCC evidence.
      if (background && previous?.token && next.token?.instanceId === previous.token.instanceId
        && next.token.revision < previous.token.revision) return;
      if (background && previous && ordered(currentBindings()) !== ordered(previous.inputs)) {
        // Do not silently rebase a dirty draft onto someone else's fresh token.
        if (JSON.stringify(next.token) !== JSON.stringify(previous.token) || ordered(next.inputs) !== ordered(previous.inputs))
          setError(normalizeProductError({ code: "SHOT_VIDEO_INPUT_CONFLICT" }).message);
        return;
      }
      // Media lives in Studio Store's draft; persisted bindings alone are production authority.
      const merged = Object.fromEntries(Object.entries(store.values).filter(([,v]) => !("assetId" in v || "assetIds" in v)));
      for (const field of fields) {
        // A same-owner Settings/Library return preserves even unsaved/cleared slots.
        // Explicit refresh still discards them and reads current OCC evidence.
        const ids = restore && restoredDraft ? assetIds(restoredDraft[field.key]) : next.inputs.filter(i => i.inputKey === field.key).sort((a,b)=>a.ordinal-b.ordinal).map(i=>i.assetId);
        if (ids.length) merged[field.key] = mediaValue(field, ids);
      }
      store.loadCreationDraft(merged, store.draftDirty, store.creationPromptProvenance);
      acceptView(own, next);
    } catch (e) { if (own === currentOwner.current && token === epoch.current && read === readEpoch.current) setError(normalizeProductError(e).message); }
    finally { if (own === currentOwner.current && token === epoch.current && read === readEpoch.current) setLoading(false); }
  }
  useEffect(() => {
    ++epoch.current; ++readEpoch.current; pendingRefresh.current = false;
    lock.current = false; setBusy(false); liveState.current = undefined; setState(undefined);
    setError(undefined); setImportReceipt(undefined); setLoading(enabled); void load(true);
    return () => { ++epoch.current; ++readEpoch.current; };
  }, [owner, enabled, pageLoading]);
  useEffect(() => {
    if (!enabled) return;
    return subscribeVideoInputInvalidation((projectId) => {
      if (projectId !== route.projectId || selfWrite.current) return;
      void load(false, true);
    });
  }, [enabled, route.projectId, owner]);

  function beginWrite() { ++readEpoch.current; lock.current = true; setLoading(false); setBusy(true); setError(undefined); }
  function finishWrite(own: string, token: number) {
    // Read generations must never control release of a write-owned lock.
    if (own !== currentOwner.current || token !== epoch.current) return;
    lock.current = false; setBusy(false);
    if (pendingRefresh.current) { pendingRefresh.current = false; void load(false, true); }
  }

  async function save() {
    if (!enabled || !view || loading || lock.current) return;
    const own = owner; const token = epoch.current; beginWrite();
    try {
      const next = await productClient.creation.inputsSave({ selection, expected: view.token, inputs });
      if (own === currentOwner.current && token === epoch.current) { acceptView(own, next); selfWrite.current = true; try { invalidateVideoInputs(route.projectId); } finally { selfWrite.current = false; } }
    } catch (e) { if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message); }
    finally { finishWrite(own, token); }
  }

  /** Persist one Library/reuse asset into an exact inputKey (+ ordinal) via OCC save. Does not create Task/Run.
   *  Merge from the current Studio draft (same base as save), not OCC view.inputs alone, so unsaved sibling slots survive. */
  async function bindAsset(inputKey: string, assetId: string): Promise<boolean> {
    if (!enabled || !view || loading || lock.current || !generator) return false;
    const field = fields.find(f => f.key === inputKey);
    if (!field || mediaKind(field) == null) return false;
    const own = owner; const token = epoch.current; beginWrite();
    try {
      // Draft `inputs` is authority for sibling keys; view.token still guards OCC.
      const others = inputs.filter(i => i.inputKey !== inputKey);
      const currentKey = inputs.filter(i => i.inputKey === inputKey).sort((a,b)=>a.ordinal-b.ordinal);
      let nextKey: VideoInputAsset[];
      if (pluralField(field)) {
        const without = currentKey.filter(i => i.assetId !== assetId);
        nextKey = [...without, { inputKey: inputKey as VideoInputKey, ordinal: without.length, assetId }]
          .map((item, ordinal) => ({ ...item, ordinal }));
      } else {
        nextKey = [{ inputKey: inputKey as VideoInputKey, ordinal: 0, assetId }];
      }
      const nextInputs = [...others, ...nextKey];
      const next = await productClient.creation.inputsSave({ selection, expected: view.token, inputs: nextInputs });
      if (own !== currentOwner.current || token !== epoch.current) return false;
      acceptView(own, next);
      draftFromBindings(next.inputs);
      selfWrite.current = true; try { invalidateVideoInputs(route.projectId); } finally { selfWrite.current = false; }
      return true;
    } catch (e) {
      if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message);
      return false;
    } finally {
      finishWrite(own, token);
    }
  }

  /** Returns the imported assets. Import itself never fills a slot; only an explicit caller (the composer's chosen slot) may bind them. */
  async function importAssets(folder: boolean): Promise<VideoInputView["assets"]> {
    if (lock.current || loading) return [];
    const own = owner; const token = epoch.current; beginWrite();
    try {
      const result = await productClient.creation.assetsImport(route.projectId, folder);
      if (own !== currentOwner.current || token !== epoch.current) return [];
      setImportReceipt(result.cancelled ? "已取消导入，未应用素材。" : `已导入 ${result.imported.length} 项；失败 ${result.failed.length} 项。请选择明确输入槽。`);
      const previous = liveState.current;
      if (previous?.owner === own) acceptView(own, { ...previous.view, assets: [...previous.view.assets, ...result.imported.filter(a => !previous.view.assets.some(b=>b.id===a.id))] });
      if (result.failed.length) setError(result.failed.map(f=>`${f.displayName}：${f.error}`).join("；"));
      // Import never fills a slot, selects a result or creates a generation.
      return result.cancelled ? [] : result.imported;
    } catch (e) { if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message); return []; }
    finally { finishWrite(own, token); }
  }
  return { enabled, loading, busy, error, importReceipt, view, dirty, save, bindAsset, refresh: load, importAssets, ready: !enabled || (!!view && !loading && !busy && !dirty && !error) };
}
