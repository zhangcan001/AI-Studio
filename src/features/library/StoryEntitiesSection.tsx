import { useCallback, useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { StoryEntity, StoryEntityKind } from "../../types/storyEntity";
import { STORY_ENTITIES_CHANGED, entityAvatarAssetId } from "../../types/storyEntity";
import { ImageAssetPicker } from "../../components/ImageAssetPicker";
import { normalizeProductError } from "../../product/errors";

const KINDS: { id: StoryEntityKind; label: string }[] = [
  { id: "character", label: "角色" },
  { id: "scene", label: "场景" },
  { id: "prop", label: "物品" },
];

/** Keep the backend reason (e.g. duplicate name) visible instead of only the generic category message. */
function entityError(cause: unknown): string {
  const normalized = normalizeProductError(cause).message;
  const raw = typeof cause === "object" && cause && "message" in cause ? String((cause as { message: unknown }).message ?? "") : "";
  return raw && raw !== normalized && !normalized.includes(raw) ? `${normalized}（${raw}）` : normalized;
}

function EntityThumb({ projectId, assetId, name }: { projectId: string; assetId: string; name: string }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let alive = true;
    let objectUrl: string | undefined;
    void productClient.library.imageGet(projectId, { kind: "asset", id: assetId })
      .then(bytes => {
        if (!alive) return;
        objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes as number[])], { type: "image/png" }));
        setUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => { alive = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [projectId, assetId]);
  return <span className="story-entity-thumb" title={name}>{url ? <img src={url} alt={name} /> : <span aria-hidden="true">…</span>}</span>;
}

/** Project-scoped Character/Scene/Prop entities with cover + image tags for storyboard chips. */
export function StoryEntitiesSection({ projectId }: { projectId: string }) {
  const [kind, setKind] = useState<StoryEntityKind | "all">("all");
  const [items, setItems] = useState<StoryEntity[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [editing, setEditing] = useState<StoryEntity | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftDesc, setDraftDesc] = useState("");
  const [draftKind, setDraftKind] = useState<StoryEntityKind>("character");
  const [draftCover, setDraftCover] = useState<string | null>(null);
  const [draftImages, setDraftImages] = useState<string[]>([]);
  const [picking, setPicking] = useState<"cover" | "images" | null>(null);
  const [creating, setCreating] = useState(false);

  const reload = useCallback(async () => {
    setBusy(true); setError(undefined);
    try {
      const list = await productClient.storyEntities.list(projectId, kind === "all" ? null : kind);
      setItems(list as StoryEntity[]);
    } catch (cause) {
      setError(entityError(cause));
    } finally { setBusy(false); }
  }, [projectId, kind]);

  useEffect(() => { void reload(); }, [reload]);

  function openCreate() {
    setCreating(true); setEditing(null);
    setDraftKind(kind === "all" ? "character" : kind);
    setDraftName(""); setDraftDesc(""); setDraftCover(null); setDraftImages([]);
  }

  function openEdit(entity: StoryEntity) {
    setCreating(false); setEditing(entity);
    setDraftKind(entity.kind); setDraftName(entity.name); setDraftDesc(entity.description);
    setDraftCover(entity.coverAssetId); setDraftImages([...entity.imageAssetIds]);
  }

  async function save() {
    setBusy(true); setError(undefined);
    try {
      const payload = { name: draftName, description: draftDesc, coverAssetId: draftCover, imageAssetIds: draftImages };
      if (editing) await productClient.storyEntities.update({ projectId, id: editing.id, ...payload });
      else await productClient.storyEntities.create({ projectId, kind: draftKind, ...payload });
      setEditing(null); setCreating(false);
      await reload(); window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
    } catch (cause) {
      setError(entityError(cause)); setBusy(false);
    }
  }

  async function remove(entity: StoryEntity) {
    if (!window.confirm(`删除「${entity.name}」？`)) return;
    setBusy(true);
    try {
      await productClient.storyEntities.delete(projectId, entity.id);
      await reload(); window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
    } catch (cause) {
      setError(entityError(cause)); setBusy(false);
    }
  }

  const formOpen = creating || editing;

  return <section className="story-entities-section" aria-label="角色与场景">
    <header className="story-entities-header">
      <h2>角色与场景</h2>
      <div className="story-entities-tabs" role="tablist" aria-label="实体类型">
        <button type="button" role="tab" aria-selected={kind === "all"} onClick={() => setKind("all")}>全部</button>
        {KINDS.map(k => <button key={k.id} type="button" role="tab" aria-selected={kind === k.id} onClick={() => setKind(k.id)}>{k.label}</button>)}
      </div>
      <button type="button" className="primary" onClick={openCreate} disabled={busy}>新建</button>
    </header>
    {error && <p role="alert">{error}</p>}
    {busy && !items.length && <p role="status">加载中…</p>}
    {!busy && !error && !items.length && <p className="library-empty">还没有{kind === "all" ? "角色 / 场景 / 物品" : KINDS.find(k => k.id === kind)?.label}。点击「新建」添加，封面和关联图片来自项目素材。</p>}
    <ul className="story-entities-list">
      {items.map(entity => <li key={entity.id} className="story-entity-card">
        {entityAvatarAssetId(entity) ? <EntityThumb projectId={projectId} assetId={entityAvatarAssetId(entity)!} name={entity.name} /> : <span className="story-entity-cover-empty">无封面</span>}
        <div className="story-entity-body">
          <strong title={entity.name}>{entity.name}</strong>
          <span className={`story-entity-kind story-entity-kind-${entity.kind}`}>{KINDS.find(k => k.id === entity.kind)?.label}</span>
          <small>{entity.imageAssetIds.length} 张图</small>
          {entity.description && <p title={entity.description}>{entity.description}</p>}
        </div>
        <div className="story-entity-actions">
          <button type="button" onClick={() => openEdit(entity)}>编辑</button>
          <button type="button" onClick={() => void remove(entity)}>删除</button>
        </div>
      </li>)}
    </ul>
    {formOpen && <div className="story-entity-editor" role="dialog" aria-label={editing ? "编辑实体" : "新建实体"}>
      <h3>{editing ? `编辑「${editing.name}」` : "新建角色 / 场景 / 物品"}</h3>
      <label>类型<select aria-label="实体类型" value={draftKind} disabled={Boolean(editing)} onChange={e => setDraftKind(e.target.value as StoryEntityKind)}>{KINDS.map(k => <option key={k.id} value={k.id}>{k.label}</option>)}</select></label>
      <label>名称<input aria-label="实体名称" value={draftName} onChange={e => setDraftName(e.target.value)} /></label>
      <label>描述<textarea aria-label="实体描述" value={draftDesc} onChange={e => setDraftDesc(e.target.value)} rows={3} /></label>
      <div className="story-entity-cover-row">
        <span>封面</span>
        {draftCover ? <EntityThumb projectId={projectId} assetId={draftCover} name="封面" /> : <span>未选择</span>}
        <button type="button" onClick={() => setPicking("cover")}>选择封面</button>
        {draftCover && <button type="button" onClick={() => setDraftCover(null)}>清除</button>}
      </div>
      <div className="story-entity-images-row">
        <span>关联图片（{draftImages.length}）</span>
        <div className="story-entity-image-chips">{draftImages.map(id => <span key={id} className="story-entity-image-chip">
          <EntityThumb projectId={projectId} assetId={id} name={id} />
          <button type="button" aria-label={`移除图片 ${id}`} onClick={() => setDraftImages(ids => ids.filter(x => x !== id))}>×</button>
        </span>)}</div>
        <button type="button" onClick={() => setPicking("images")}>添加图片</button>
      </div>
      <footer>
        <button type="button" onClick={() => void save()} disabled={busy || !draftName.trim()}>保存</button>
        <button type="button" onClick={() => { setCreating(false); setEditing(null); }}>取消</button>
      </footer>
    </div>}
    {picking && <ImageAssetPicker
      projectId={projectId}
      open
      title={picking === "cover" ? "选择封面" : "添加关联图片"}
      excludeIds={picking === "images" ? draftImages : []}
      onSelect={id => {
        if (picking === "cover") setDraftCover(id);
        else setDraftImages(ids => ids.includes(id) ? ids : [...ids, id]);
        setPicking(null);
      }}
      onClose={() => setPicking(null)}
    />}
  </section>;
}
