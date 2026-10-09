import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { GeneratorOption } from "../../product/types";
import type { VideoInputAsset, VideoInputKey, VideoInputView } from "../../types/shotVideoInput";
import { useStudioStore } from "../../stores/studioStore";
import { assetIds, mediaKind, mediaValue, type CreateRoute } from "./createModel";
import type { GenerationValues } from "../../types/generation";

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
  return { enabled, loading, busy, error, importReceipt, view, dirty, save, refresh: load, importAssets, ready: !enabled || (!!view && !loading && !busy && !dirty && !error) };
}
