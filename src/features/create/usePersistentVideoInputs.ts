import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
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
  const epoch = useRef(0); const lock = useRef(false);
  const values = useStudioStore(state => state.values);
  const [state, setState] = useState<{ owner: string; view: VideoInputView }>();
  const [loading, setLoading] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState<string>();
  const [importReceipt, setImportReceipt] = useState<string>();
  const selection = { projectId: route.projectId, shotId: route.shotId ?? "", selectionRef: generator?.selectionRef ?? "" };
  const view = state?.owner === owner ? state.view : undefined;
  const fields = generator?.fields.filter(field => mediaKind(field)) ?? [];
  const inputs: VideoInputAsset[] = fields.flatMap(field => assetIds(values[field.key]).map((assetId, ordinal) => ({ inputKey: field.key as VideoInputKey, ordinal, assetId })));
  const ordered = (bindings: VideoInputAsset[]) => JSON.stringify([...bindings].sort((a,b) => a.inputKey.localeCompare(b.inputKey) || a.ordinal-b.ordinal));
  const dirty = enabled && !!view && ordered(inputs) !== ordered(view.inputs);

  function draftFromBindings(bindings: VideoInputAsset[]) {
    const store = useStudioStore.getState();
    const merged = Object.fromEntries(Object.entries(store.values).filter(([,v]) => !("assetId" in v || "assetIds" in v)));
    for (const field of fields) {
      const ids = bindings.filter(i => i.inputKey === field.key).sort((a,b)=>a.ordinal-b.ordinal).map(i=>i.assetId);
      if (ids.length) merged[field.key] = mediaValue(field, ids);
    }
    store.loadCreationDraft(merged, store.draftDirty, store.creationPromptProvenance);
  }

  async function load(restore = false) {
    if (!enabled || pageLoading) return;
    const token = ++epoch.current; const own = owner; setLoading(true); setError(undefined);
    try {
      const next = await productClient.creation.inputsGet(selection);
      if (own !== currentOwner.current || token !== epoch.current) return;
      const store = useStudioStore.getState();
      // Media lives in Studio Store's draft; persisted bindings alone are production authority.
      const merged = Object.fromEntries(Object.entries(store.values).filter(([,v]) => !("assetId" in v || "assetIds" in v)));
      for (const field of fields) {
        // A same-owner Settings/Library return preserves even unsaved/cleared slots.
        // Explicit refresh still discards them and reads current OCC evidence.
        const ids = restore && restoredDraft ? assetIds(restoredDraft[field.key]) : next.inputs.filter(i => i.inputKey === field.key).sort((a,b)=>a.ordinal-b.ordinal).map(i=>i.assetId);
        if (ids.length) merged[field.key] = mediaValue(field, ids);
      }
      store.loadCreationDraft(merged, store.draftDirty, store.creationPromptProvenance);
      setState({ owner: own, view: next });
    } catch (e) { if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message); }
    finally { if (own === currentOwner.current && token === epoch.current) setLoading(false); }
  }
  useEffect(() => { lock.current = false; setBusy(false); setState(undefined); setError(undefined); setImportReceipt(undefined); setLoading(enabled); void load(true); return () => { ++epoch.current; }; }, [owner, enabled, pageLoading]);

  async function save() {
    if (!enabled || !view || loading || lock.current) return;
    const own = owner; const token = epoch.current; lock.current = true; setBusy(true); setError(undefined);
    try {
      const next = await productClient.creation.inputsSave({ selection, expected: view.token, inputs });
      if (own === currentOwner.current && token === epoch.current) setState({ owner: own, view: next });
    } catch (e) { if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message); }
    finally { if (own === currentOwner.current && token === epoch.current) { lock.current = false; setBusy(false); } }
  }

  /** Persist one Library/reuse asset into an exact inputKey (+ ordinal) via OCC save. Does not create Task/Run.
   *  Merge from the current Studio draft (same base as save), not OCC view.inputs alone, so unsaved sibling slots survive. */
  async function bindAsset(inputKey: string, assetId: string): Promise<boolean> {
    if (!enabled || !view || loading || lock.current || !generator) return false;
    const field = fields.find(f => f.key === inputKey);
    if (!field || mediaKind(field) == null) return false;
    const own = owner; const token = epoch.current; lock.current = true; setBusy(true); setError(undefined);
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
      setState({ owner: own, view: next });
      draftFromBindings(next.inputs);
      return true;
    } catch (e) {
      if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message);
      return false;
    } finally {
      if (own === currentOwner.current && token === epoch.current) { lock.current = false; setBusy(false); }
    }
  }

  async function importAssets(folder: boolean) {
    if (lock.current) return;
    const own = owner; const token = epoch.current; lock.current = true; setBusy(true); setError(undefined);
    try {
      const result = await productClient.creation.assetsImport(route.projectId, folder);
      if (own !== currentOwner.current || token !== epoch.current) return;
      setImportReceipt(result.cancelled ? "已取消导入，未应用素材。" : `已导入 ${result.imported.length} 项；失败 ${result.failed.length} 项。请选择明确输入槽。`);
      setState(previous => previous?.owner === own ? { owner: own, view: { ...previous.view, assets: [...previous.view.assets, ...result.imported.filter(a => !previous.view.assets.some(b=>b.id===a.id))] } } : previous);
      if (result.failed.length) setError(result.failed.map(f=>`${f.displayName}：${f.error}`).join("；"));
      // Import never fills a slot, selects a result or creates a generation.
    } catch (e) { if (own === currentOwner.current && token === epoch.current) setError(normalizeProductError(e).message); }
    finally { if (own === currentOwner.current && token === epoch.current) { lock.current = false; setBusy(false); } }
  }
  return { enabled, loading, busy, error, importReceipt, view, dirty, save, bindAsset, refresh: load, importAssets, ready: !enabled || (!!view && !loading && !busy && !dirty && !error) };
}
