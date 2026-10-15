import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { StoryEntity } from "../../types/storyEntity";

const KIND_LABEL: Record<string, string> = { character: "角色", scene: "场景", prop: "物品" };

/** Avatar chips for story entities bound to a shot (storyboard cards / detail). */
export function ShotEntityChips({ projectId, shotId, editable = false }: { projectId: string; shotId: string; editable?: boolean }) {
  const [entities, setEntities] = useState<StoryEntity[]>([]);
  const [all, setAll] = useState<StoryEntity[]>([]);
  const [busy, setBusy] = useState(false);
  const [urls, setUrls] = useState<Record<string, string>>({});

  useEffect(() => {
    let alive = true;
    const listForShot = productClient.storyEntities?.listForShot;
    const listAll = productClient.storyEntities?.list;
    if (typeof listForShot === "function") void listForShot(projectId, shotId).then(list => { if (alive) setEntities(list); }).catch(() => undefined);
    if (editable && typeof listAll === "function") void listAll(projectId, null).then(list => { if (alive) setAll(list); }).catch(() => undefined);
    return () => { alive = false; };
  }, [projectId, shotId, editable]);

  useEffect(() => {
    let alive = true;
    const created: string[] = [];
    for (const entity of entities) {
      const id = entity.coverAssetId;
      if (!id || urls[id]) continue;
      void productClient.library.imageGet(projectId, { kind: "asset", id }).then(bytes => {
        if (!alive) return;
        const url = URL.createObjectURL(new Blob([new Uint8Array(bytes as number[])], { type: "image/png" }));
        created.push(url);
        setUrls(m => ({ ...m, [id]: url }));
      }).catch(() => undefined);
    }
    return () => { alive = false; created.forEach(u => URL.revokeObjectURL(u)); };
  }, [projectId, entities.map(e => e.coverAssetId).join("|")]);

  async function toggle(entityId: string) {
    if (!editable || busy) return;
    setBusy(true);
    const ids = entities.some(e => e.id === entityId)
      ? entities.filter(e => e.id !== entityId).map(e => e.id)
      : [...entities.map(e => e.id), entityId];
    try {
      const next = await productClient.storyEntities.bindShot(projectId, shotId, ids);
      setEntities(next);
    } finally { setBusy(false); }
  }

  if (!entities.length && !editable) return null;
  return <div className="shot-entity-chips" aria-label="绑定角色与场景">
    {entities.map(entity => {
      const cover = entity.coverAssetId;
      return <button key={entity.id} type="button" className="shot-entity-chip" title={`${KIND_LABEL[entity.kind] ?? entity.kind} · ${entity.name}`}
        disabled={!editable || busy} onClick={() => void toggle(entity.id)}>
        {cover && urls[cover] ? <img src={urls[cover]} alt="" /> : <span className="shot-entity-chip-glyph">{entity.name.slice(0, 1)}</span>}
        <small>{entity.name}</small>
      </button>;
    })}
    {editable && <details className="shot-entity-bind">
      <summary>绑定实体</summary>
      <ul>{all.map(entity => <li key={entity.id}>
        <label><input type="checkbox" checked={entities.some(e => e.id === entity.id)} disabled={busy}
          onChange={() => void toggle(entity.id)} />{KIND_LABEL[entity.kind]} · {entity.name}</label>
      </li>)}</ul>
    </details>}
  </div>;
}
