import { useCallback, useEffect, useRef, useState } from "react";
import { ImageAssetPicker } from "../../components/ImageAssetPicker";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { entityAvatarAssetId, STORY_ENTITIES_CHANGED } from "../../types/storyEntity";
import type { StoryEntity, StoryEntityKind } from "../../types/storyEntity";

const KIND_LABEL: Record<StoryEntityKind, string> = {
  character: "角色",
  scene: "场景",
  prop: "道具",
  composition: "构图",
};

function entityError(cause: unknown): string {
  const normalized = normalizeProductError(cause).message;
  const raw = typeof cause === "object" && cause && "message" in cause
    ? String((cause as { message: unknown }).message ?? "")
    : "";
  return raw && raw !== normalized && !normalized.includes(raw) ? `${normalized}（${raw}）` : normalized;
}

function filenameStem(value: string): string {
  const filename = value.split(/[\\/]/).pop()?.trim() ?? "";
  const dot = filename.lastIndexOf(".");
  return (dot > 0 ? filename.slice(0, dot) : filename).trim();
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

/** A dedicated, project-scoped view for one entity kind. */
export function StoryEntityCategorySection({ projectId, kind }: { projectId: string; kind: StoryEntityKind }) {
  const label = KIND_LABEL[kind];
  const [items, setItems] = useState<StoryEntity[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<StoryEntity | null>(null);
  const [creating, setCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftDesc, setDraftDesc] = useState("");
  const [draftCover, setDraftCover] = useState<string | null>(null);
  const [draftImages, setDraftImages] = useState<string[]>([]);
  const [picking, setPicking] = useState<"cover" | "images" | null>(null);
  const mounted = useRef(false);
  const requestEpoch = useRef(0);

  const reload = useCallback(async () => {
    const epoch = ++requestEpoch.current;
    setLoading(true);
    setError(undefined);
    try {
      const list = await productClient.storyEntities.list(projectId, kind);
      if (mounted.current && epoch === requestEpoch.current) setItems(list as StoryEntity[]);
    } catch (cause) {
      if (mounted.current && epoch === requestEpoch.current) setError(entityError(cause));
    } finally {
      if (mounted.current && epoch === requestEpoch.current) setLoading(false);
    }
  }, [projectId, kind]);

  useEffect(() => {
    mounted.current = true;
    void reload();
    return () => { mounted.current = false; ++requestEpoch.current; };
  }, [reload]);

  function openCreate() {
    setCreating(true);
    setEditing(null);
    setDraftName("");
    setDraftDesc("");
    setDraftCover(null);
    setDraftImages([]);
    setError(undefined);
  }

  function openEdit(entity: StoryEntity) {
    setCreating(false);
    setEditing(entity);
    setDraftName(entity.name);
    setDraftDesc(entity.description);
    setDraftCover(entity.coverAssetId);
    setDraftImages([...entity.imageAssetIds]);
    setError(undefined);
  }

  async function save() {
    if (!draftName.trim() || busy) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const payload = {
        name: draftName.trim(),
        description: draftDesc.trim(),
        coverAssetId: draftCover,
        imageAssetIds: draftImages,
      };
      if (editing) await productClient.storyEntities.update({ projectId, id: editing.id, ...payload });
      else await productClient.storyEntities.create({ projectId, kind, ...payload });
      window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
      setCreating(false);
      setEditing(null);
      await reload();
      if (mounted.current) setNotice(`${editing ? "已更新" : "已创建"}${label}「${payload.name}」。`);
    } catch (cause) {
      if (mounted.current) setError(entityError(cause));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  async function importImages() {
    if (busy) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    let created = 0;
    const failures: string[] = [];
    try {
      const result = await productClient.storyEntities.importImages(projectId);
      if (result.cancelled) return;
      failures.push(...result.failed.map(item => `${item.displayName}：${item.error}`));
      for (const asset of result.imported) {
        const name = filenameStem(asset.originalName || asset.name);
        if (!name) {
          failures.push(`${asset.originalName || asset.name}：无法从文件名生成名称。`);
          continue;
        }
        try {
          await productClient.storyEntities.create({
            projectId,
            kind,
            name,
            description: "",
            coverAssetId: asset.id,
            imageAssetIds: [asset.id],
          });
          created += 1;
        } catch (cause) {
          failures.push(`${asset.originalName || asset.name}：${entityError(cause)}`);
        }
      }
      if (created) window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
      if (!mounted.current) return;
      await reload();
      if (!mounted.current) return;
      const details = failures.slice(0, 4).join("；");
      const remaining = failures.length > 4 ? `；另有 ${failures.length - 4} 项失败` : "";
      setNotice(`批量导入完成：成功创建 ${created} 个${label}。${failures.length ? `失败 ${failures.length} 项：${details}${remaining}。已导入的图片仍保留在媒体库。` : ""}`);
    } catch (cause) {
      if (mounted.current) setError(entityError(cause));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  async function remove(entity: StoryEntity) {
    if (busy || !window.confirm(`删除「${entity.name}」？这不会删除原始图片素材。`)) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await productClient.storyEntities.delete(projectId, entity.id);
      window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
      await reload();
      if (mounted.current) setNotice(`已删除${label}「${entity.name}」。`);
    } catch (cause) {
      if (mounted.current) setError(entityError(cause));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  const visibleItems = items.filter(item => !search.trim() || `${item.name} ${item.description}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const formOpen = creating || Boolean(editing);

  return <section className="story-entities-section" aria-label={`${label}管理`}>
    <header className="story-entities-header">
      <div>
        <h2>{label}</h2>
        <p className="story-entity-import-hint">可批量选择多张图片；每张图片自动创建一项{label}，名称取文件名（不含扩展名），图片同时作为封面和关联图。</p>
      </div>
      <div className="story-entity-header-actions">
        <button type="button" onClick={() => void importImages()} disabled={busy || loading}>{busy ? "正在处理…" : `批量导入${label}图片`}</button>
        <button type="button" onClick={openCreate} disabled={busy || loading}>新建{label}</button>
        <button type="button" onClick={() => void reload()} disabled={busy || loading}>刷新</button>
      </div>
    </header>
    <label className="story-entity-search">搜索{label}
      <input type="search" aria-label={`搜索${label}`} value={search} onChange={event => setSearch(event.target.value)} placeholder={`按${label}名称或描述筛选`} />
    </label>
    {error && <p role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {loading && !items.length && <p role="status">加载中…</p>}
    {!loading && !error && !visibleItems.length && <p className="library-empty">{items.length ? "没有符合搜索条件的项目。" : `还没有${label}。可以新建，也可以批量导入图片。`}</p>}
    <ul className="story-entities-list">
      {visibleItems.map(entity => <li key={entity.id} className="story-entity-card">
        {entityAvatarAssetId(entity) ? <EntityThumb projectId={projectId} assetId={entityAvatarAssetId(entity)!} name={entity.name} /> : <span className="story-entity-cover-empty">无封面</span>}
        <div className="story-entity-body">
          <strong title={entity.name}>{entity.name}</strong>
          <small>{entity.imageAssetIds.length} 张图</small>
          {entity.description && <p title={entity.description}>{entity.description}</p>}
        </div>
        <div className="story-entity-actions">
          <button type="button" onClick={() => openEdit(entity)} disabled={busy}>编辑</button>
          <button type="button" onClick={() => void remove(entity)} disabled={busy}>删除</button>
        </div>
      </li>)}
    </ul>
    {formOpen && <div className="story-entity-editor" role="dialog" aria-label={editing ? `编辑${label}` : `新建${label}`}>
      <h3>{editing ? `编辑「${editing.name}」` : `新建${label}`}</h3>
      <label>名称<input aria-label={`${label}名称`} value={draftName} onChange={event => setDraftName(event.target.value)} /></label>
      <label>描述<textarea aria-label={`${label}描述`} value={draftDesc} onChange={event => setDraftDesc(event.target.value)} rows={3} /></label>
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
          <button type="button" aria-label={`移除图片 ${id}`} onClick={() => setDraftImages(ids => ids.filter(value => value !== id))}>×</button>
        </span>)}</div>
        <button type="button" onClick={() => setPicking("images")}>添加图片</button>
      </div>
      <footer>
        <button type="button" onClick={() => void save()} disabled={busy || !draftName.trim()}>保存</button>
        <button type="button" onClick={() => { setCreating(false); setEditing(null); }} disabled={busy}>取消</button>
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
