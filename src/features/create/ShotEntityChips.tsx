import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import type { StoryEntity } from "../../types/storyEntity";
import { entityAvatarAssetId } from "../../types/storyEntity";
import { normalizeProductError } from "../../product/errors";
import { useLibraryImageUrl } from "./CreateMediaPreview";

const KIND_LABEL: Record<string, string> = { character: "角色", scene: "场景", prop: "物品" };

/** Local lifecycle coordination only; the typed backend remains the binding authority. */
export function useShotEntityBindings(projectId: string | undefined, shotId: string | undefined, revision = 0) {
  const key = JSON.stringify([projectId, shotId]);
  const active = useRef(key), epoch = useRef(0), reads = useRef(0);
  if (active.current !== key) { active.current = key; ++epoch.current; }
  const [retry, setRetry] = useState(0);
  const [view, setView] = useState<{ key: string; entities: StoryEntity[]; ready: boolean; error?: string }>({ key, entities: [], ready: false });
  const [working, setWorking] = useState<{ key: string; epoch: number } | null>(null);
  const lock = useRef<typeof working>(null);
  useEffect(() => () => { ++epoch.current; }, [key]);
  useEffect(() => {
    const read = ++reads.current, owner = epoch.current;
    const current = () => active.current === key && epoch.current === owner && read === reads.current;
    setView({ key, entities: [], ready: false });
    const get = productClient.storyEntities?.listForShot;
    if (projectId && shotId && typeof get === "function") {
      void Promise.resolve().then(() => get(projectId, shotId)).then(entities => {
        if (current()) setView({ key, entities: entities ?? [], ready: true });
      }).catch(cause => { if (current()) setView({ key, entities: [], ready: false, error: normalizeProductError(cause).message }); });
    }
    return () => { ++reads.current; };
  }, [key, projectId, shotId, revision, retry]);
  const currentView = view.key === key ? view : { entities: [], ready: false, error: undefined };
  const busy = working?.key === key && working.epoch === epoch.current;
  async function replace(ids: string[]) {
    if (!projectId || !shotId || !currentView.ready || (lock.current?.key === key && lock.current.epoch === epoch.current)) return false;
    const operation = { key, epoch: epoch.current };
    lock.current = operation; setWorking(operation); ++reads.current;
    const current = () => active.current === key && epoch.current === operation.epoch;
    try {
      const entities = await productClient.storyEntities.bindShot(projectId, shotId, ids);
      if (!current()) return false;
      ++reads.current; setView({ key, entities: entities ?? [], ready: true }); return true;
    } catch (cause) {
      if (current()) setView(previous => ({ ...previous, error: normalizeProductError(cause).message }));
      return false;
    } finally {
      if (lock.current === operation) lock.current = null;
      if (current()) setWorking(null);
    }
  }
  return { ...currentView, busy, replace, reload: () => setRetry(value => value + 1) };
}

function EntityAvatar({ projectId, entity }: { projectId: string; entity: StoryEntity }) {
  const url = useLibraryImageUrl(projectId, entityAvatarAssetId(entity));
  return url ? <img src={url} alt="" /> : <span className="shot-entity-chip-glyph">{entity.name.slice(0, 1)}</span>;
}

/** Avatar chips for story entities bound to a shot (storyboard cards / detail). */
export function ShotEntityChips({ projectId, shotId, editable = false }: { projectId: string; shotId: string; editable?: boolean }) {
  const binding = useShotEntityBindings(projectId, shotId);
  const { entities, busy } = binding;
  const [all, setAll] = useState<StoryEntity[]>([]);

  useEffect(() => {
    let alive = true;
    const listAll = productClient.storyEntities?.list;
    setAll([]);
    if (editable && typeof listAll === "function") void listAll(projectId, null).then(list => { if (alive) setAll(list); }).catch(() => undefined);
    return () => { alive = false; };
  }, [projectId, shotId, editable]);

  async function toggle(entityId: string) {
    if (!editable || busy || !binding.ready) return;
    const ids = entities.some(e => e.id === entityId)
      ? entities.filter(e => e.id !== entityId).map(e => e.id)
      : [...entities.map(e => e.id), entityId];
    await binding.replace(ids);
  }

  if (!entities.length && !editable) return null;
  return <div className="shot-entity-chips" aria-label="绑定角色与场景">
    {entities.map(entity => {
      return <button key={entity.id} type="button" className="shot-entity-chip" title={`${KIND_LABEL[entity.kind] ?? entity.kind} · ${entity.name}`}
        disabled={!editable || busy || !binding.ready} onClick={() => void toggle(entity.id)}>
        <EntityAvatar projectId={projectId} entity={entity} />
        <small>{entity.name}</small>
      </button>;
    })}
    {editable && <details className="shot-entity-bind">
      <summary>绑定实体</summary>
      <ul>{all.map(entity => <li key={entity.id}>
        <label><input type="checkbox" checked={entities.some(e => e.id === entity.id)} disabled={busy || !binding.ready}
          onChange={() => void toggle(entity.id)} />{KIND_LABEL[entity.kind]} · {entity.name}</label>
      </li>)}</ul>
    </details>}
    {binding.error && <p role="alert">{binding.error}<button type="button" disabled={busy} onClick={binding.reload}>重试读取绑定</button></p>}
  </div>;
}
