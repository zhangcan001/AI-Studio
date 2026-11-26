import { LibraryThumbnail, LibraryThumbnailScope } from "./LibraryThumbnail";
import { useCallback, useEffect, useRef, useState } from "react";
import { ImageAssetPicker } from "../../components/ImageAssetPicker";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns } from "../../product/runInvalidation";
import { invalidateVideoInputs } from "../../product/videoInputInvalidation";
import { appearanceLanguageHint } from "../../product/promptLanguage";
import { entityAvatarAssetId, STORY_ENTITIES_CHANGED } from "../../types/storyEntity";
import type { StoryEntity, StoryEntityKind } from "../../types/storyEntity";
import { storyboardDraftKeys, useStudioStore } from "../../stores/studioStore";

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
  return <span className="story-entity-thumb" title={name}><LibraryThumbnail projectId={projectId} item={{resourceRef:{kind:"asset",id:assetId},title:name,subtype:"image",thumbnailAvailable:true,createdAt:"",updatedAt:""}} /></span>;
}

/** A dedicated, project-scoped view for one entity kind. */
export function StoryEntityCategorySection({ projectId, kind, onDirtyChange }: { projectId: string; kind: StoryEntityKind; onDirtyChange?: (dirty: boolean) => void }) {
  const label = KIND_LABEL[kind];
  const [items, setItems] = useState<StoryEntity[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [importMode, setImportMode] = useState<"separate" | "group">("separate");
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
  const scopeEpoch = useRef(0);
  const editorSessionRef = useRef(0);
  const coverRevisionRef = useRef(0);
  const importInFlightRef = useRef<object | null>(null);

  const closeEditor = () => {
    editorSessionRef.current += 1;
    coverRevisionRef.current += 1;
    setCreating(false);
    setEditing(null);
    setPicking(null);
  };

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
    editorSessionRef.current += 1;
    coverRevisionRef.current += 1;
    importInFlightRef.current = null;
    setCreating(false);
    setEditing(null);
    setPicking(null);
    setSaving(false);
    setBusy(false);
    void reload();
    return () => {
      mounted.current = false;
      ++requestEpoch.current;
      ++scopeEpoch.current;
      editorSessionRef.current += 1;
      coverRevisionRef.current += 1;
    };
  }, [reload]);

  const editorDirty=(creating||Boolean(editing)) && JSON.stringify([draftName,draftDesc,draftCover,draftImages])!==JSON.stringify(editing?[editing.name,editing.description,editing.coverAssetId,editing.imageAssetIds]:["","",null,[]]);
  const allowReplace=()=>!editorDirty||window.confirm("当前编辑尚未保存，确定放弃并切换？");
  function resetCreate() {
    editorSessionRef.current += 1;
    coverRevisionRef.current += 1;
    setCreating(true);
    setEditing(null);
    setDraftName("");
    setDraftDesc("");
    setDraftCover(null);
    setDraftImages([]);
    setError(undefined);
    setNotice(undefined);
  }

  function openCreate(){if(allowReplace())resetCreate();}
  function openEdit(entity: StoryEntity) {
    if(editing?.id===entity.id&&!creating)return;
    if(!allowReplace())return;
    editorSessionRef.current += 1;
    coverRevisionRef.current += 1;
    setCreating(false);
    setEditing(entity);
    setDraftName(entity.name);
    setDraftDesc(entity.description);
    setDraftCover(entity.coverAssetId);
    setDraftImages([...entity.imageAssetIds]);
    setError(undefined);
    setNotice(undefined);
  }

  async function save() {
    if (!draftName.trim() || busy) return;
    if(editing&&draftName.trim()!==editing.name){
      const store=useStudioStore.getState();
      if(storyboardDraftKeys(store,projectId).length||store.draftDirty){
        setError("请先保存或放弃当前项目未保存的镜头草稿，再修改名称。保存改名会同步更新镜头正式提示词，历史批次和生成记录不会改写。");return;
      }
    }
    const scope = scopeEpoch.current;
    setSaving(true);
    setBusy(true);
    setPicking(null);
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
      window.dispatchEvent(new CustomEvent(STORY_ENTITIES_CHANGED, { detail: { projectId } }));
      if (!mounted.current || scope !== scopeEpoch.current) return;
      closeEditor();
      await reload();
      if (mounted.current && scope === scopeEpoch.current) setNotice(`${editing ? "已更新" : "已创建"}${label}「${payload.name}」。${editing&&editing.name!==payload.name?"镜头正式提示词中的精确 @引用已同步；历史批次与结果保持不变。":""}`);
    } catch (cause) {
      if (mounted.current && scope === scopeEpoch.current) setError(entityError(cause));
    } finally {
      if (mounted.current && scope === scopeEpoch.current) {
        setSaving(false);
        setBusy(false);
      }
    }
  }

  async function importImages() {
    if (busy || (editorDirty && !window.confirm("当前编辑尚未保存，确定放弃并导入图片？"))) return;
    const scope = scopeEpoch.current;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    let created = 0;
    let notified = false;
    const notifyEntitiesChanged = () => {
      if (created > 0 && !notified) {
        notified = true;
        window.dispatchEvent(new CustomEvent(STORY_ENTITIES_CHANGED, { detail: { projectId } }));
      }
    };
    const failures: string[] = [];
    try {
      const result = await productClient.storyEntities.importImages(projectId);
      if (result.imported.length > 0) {
        invalidateRuns(projectId);
        invalidateVideoInputs(projectId);
      }
      if (!mounted.current || scope !== scopeEpoch.current || result.cancelled) return;
      failures.push(...result.failed.map(item => `${item.displayName}：${item.error}`));
      if (importMode === "group" && result.imported.length) {
        resetCreate();
        setDraftName(filenameStem(result.imported[0].originalName || result.imported[0].name));
        setDraftCover(result.imported[0].id);
        setDraftImages(result.imported.map(asset => asset.id));
        setNotice(`已导入 ${result.imported.length} 张图，请确认名称与关联图片后保存为同一${label}。${failures.join("；")}`);
        return;
      }
      for (const asset of result.imported) {
        if (!mounted.current || scope !== scopeEpoch.current) {
          notifyEntitiesChanged();
          return;
        }
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
      notifyEntitiesChanged();
      if (!mounted.current || scope !== scopeEpoch.current) return;
      await reload();
      if (!mounted.current || scope !== scopeEpoch.current) return;
      const details = failures.slice(0, 4).join("；");
      const remaining = failures.length > 4 ? `；另有 ${failures.length - 4} 项失败` : "";
      setNotice(`批量导入完成：成功创建 ${created} 个${label}。${failures.length ? `失败 ${failures.length} 项：${details}${remaining}。已导入的图片仍保留在媒体库。` : ""}`);
    } catch (cause) {
      if (mounted.current && scope === scopeEpoch.current) setError(entityError(cause));
    } finally {
      notifyEntitiesChanged();
      if (mounted.current && scope === scopeEpoch.current) setBusy(false);
    }
  }

  async function remove(entity: StoryEntity) {
    if (busy || !window.confirm(`删除「${entity.name}」？这不会删除原始图片素材。`)) return;
    const scope = scopeEpoch.current;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await productClient.storyEntities.delete(projectId, entity.id);
      window.dispatchEvent(new CustomEvent(STORY_ENTITIES_CHANGED, { detail: { projectId } }));
      if (!mounted.current || scope !== scopeEpoch.current) return;
      await reload();
      if (mounted.current && scope === scopeEpoch.current) setNotice(`已删除${label}「${entity.name}」。`);
    } catch (cause) {
      if (mounted.current && scope === scopeEpoch.current) setError(entityError(cause));
    } finally {
      if (mounted.current && scope === scopeEpoch.current) setBusy(false);
    }
  }

  const visibleItems = items.filter(item => !search.trim() || `${item.name} ${item.description}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const formOpen = creating || Boolean(editing);
  const langHint = kind !== "composition" ? appearanceLanguageHint(draftDesc) : undefined;

  const latestSaveRef = useRef(save);
  latestSaveRef.current = save;

  useEffect(() => { onDirtyChange?.(editorDirty); return () => onDirtyChange?.(false); }, [editorDirty, onDirtyChange]);
  useEffect(() => {
    if (!formOpen || picking) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter" && !busy && draftName.trim()) {
        e.preventDefault();
        void latestSaveRef.current();
        return;
      }
      if (e.key === "Escape" && !busy && allowReplace()) {
        e.preventDefault();
        closeEditor();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [formOpen, picking, busy, editorDirty, draftName]);

  return <LibraryThumbnailScope><section className="story-entities-section" aria-label={`${label}管理`}>
    <header className="story-entities-header">
      <div>
        <h2>{label}</h2>
        <p className="story-entity-import-hint">可批量选择多张图片；可每张图片新建一项，也可将本次所选图片归为同一{label}，确认后保存。不按文件名猜测合并。</p>
      </div>
      <div className="story-entity-header-actions">
        <label>导入方式<select aria-label="实体图片导入方式" value={importMode} disabled={busy} onChange={e => setImportMode(e.target.value as "separate" | "group")}><option value="separate">每张图新建一项</option><option value="group">所选图片归为同一项（确认后保存）</option></select></label>
        <button type="button" onClick={() => void importImages()} disabled={busy || loading}>{busy ? "正在处理…" : `批量导入${label}图片`}</button>
        <button type="button" onClick={() => openCreate()} disabled={busy || loading}>新建{label}</button>
        <button type="button" onClick={() => void reload()} disabled={busy || loading}>刷新</button>
      </div>
    </header>
    <label className="story-entity-search">搜索{label}
      <input type="search" aria-label={`搜索${label}`} value={search} onChange={event => setSearch(event.target.value)} placeholder={`按${label}名称或描述筛选`} />
    </label>
    {!formOpen && error && <p role="alert">{error}</p>}
    {!formOpen && notice && <p role="status">{notice}</p>}
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
    {formOpen && <>
      <div className="story-entity-backdrop" role="presentation" onClick={() => { if (!busy && allowReplace()) closeEditor(); }} />
      <div className="story-entity-editor" role="dialog" aria-label={editing ? `编辑${label}` : `新建${label}`}>
        <h3>{editing ? `编辑「${editing.name}」` : `新建${label}`}</h3>
        {error && <p role="alert">{error}</p>}
        {notice && <p role="status">{notice}</p>}
        <label>名称<input aria-label={`${label}名称`} disabled={saving} value={draftName} onChange={event => setDraftName(event.target.value)} /></label>
        <label>描述<textarea aria-label={`${label}描述`} disabled={saving} value={draftDesc} placeholder={kind === "composition" ? "描述画面构图、景别或镜头运动关系" : "建议使用英文填写外貌/场景/材质特征，生成时将自动注入提示词"} onChange={event => setDraftDesc(event.target.value)} rows={3} /></label>
        {langHint && <small className="story-entity-lang-hint" aria-label="描述语言建议">{langHint}</small>}
        <div className="story-entity-cover-row">
          <span>封面</span>
          {draftCover ? <EntityThumb projectId={projectId} assetId={draftCover} name="封面" /> : <span>未选择</span>}
          <button type="button" disabled={saving} onClick={() => setPicking("cover")}>选择封面</button>
          {draftCover && <button type="button" disabled={saving} onClick={() => { coverRevisionRef.current += 1; setDraftCover(null); }}>清除</button>}
        </div>
        <div className="story-entity-images-row">
          <span>关联图片（{draftImages.length}）</span>
          <div className="story-entity-image-chips">{draftImages.map(id => <span key={id} className="story-entity-image-chip">
            <EntityThumb projectId={projectId} assetId={id} name={id} />
            {draftCover === id ? <small className="story-entity-cover-tag">封面</small> : <button type="button" className="story-entity-set-cover" aria-label={`将图片 ${id} 设为封面`} disabled={saving} onClick={() => { coverRevisionRef.current += 1; setDraftCover(id); }}>设封面</button>}
            <button type="button" className="story-entity-remove-img" aria-label={`移除图片 ${id}`} disabled={saving} onClick={() => { setDraftImages(ids => ids.filter(value => value !== id)); if (draftCover === id) { coverRevisionRef.current += 1; setDraftCover(null); } }}>×</button>
          </span>)}</div>
          <button type="button" disabled={saving} onClick={() => setPicking("images")}>添加图片</button>
        </div>
        <footer>
          <button type="button" className="primary" onClick={() => void save()} disabled={busy || !draftName.trim()}>保存</button>
          <button type="button" onClick={() => { if(allowReplace()) closeEditor(); }} disabled={busy}>取消</button>
        </footer>
      </div>
    </>}
    {picking && <ImageAssetPicker
      projectId={projectId}
      open
      title={picking === "cover" ? "选择封面" : "添加关联图片"}
      excludeIds={picking === "images" ? draftImages : []}
      onSelect={id => {
        if (saving) return;
        if (picking === "cover") {
          coverRevisionRef.current += 1;
          setDraftCover(id);
        } else {
          setDraftImages(ids => ids.includes(id) ? ids : [...ids, id]);
          if (!draftCover) {
            coverRevisionRef.current += 1;
            setDraftCover(id);
          }
        }
        setPicking(null);
      }}
      onImportFile={saving || busy ? undefined : () => {
        if (saving || busy || importInFlightRef.current) return;
        const importOp = {};
        importInFlightRef.current = importOp;
        const target = picking;
        const scope = scopeEpoch.current;
        const editorSession = editorSessionRef.current;
        const coverRevision = target === "cover" ? ++coverRevisionRef.current : coverRevisionRef.current;
        void (async () => {
          setBusy(true); setError(undefined);
          try {
            const res = await productClient.storyEntities.importImages(projectId);
            if (res.imported.length > 0) {
              invalidateRuns(projectId);
              invalidateVideoInputs(projectId);
            }
            if (!mounted.current || scope !== scopeEpoch.current || editorSession !== editorSessionRef.current) return;
            if (res.cancelled) return;
            const failedDetails = res.failed.map(item => `${item.displayName}：${item.error}`).join("；");
            if (!res.imported.length) {
              if (failedDetails) setError(failedDetails);
              return;
            }
            const ids = res.imported.map(a => a.id);
            if (target === "cover") {
              if (coverRevision !== coverRevisionRef.current) return;
              setDraftCover(ids[0]);
              setDraftImages(prev => [...prev, ...ids.filter(id => !prev.includes(id))]);
            } else {
              setDraftImages(prev => [...prev, ...ids.filter(id => !prev.includes(id))]);
              if (coverRevision === coverRevisionRef.current) setDraftCover(prev => prev ?? ids[0]);
            }
            const summary = target === "cover" && ids.length > 1
              ? `已从电脑导入 ${ids.length} 张图片，已将第 1 张设为封面并全部加入关联图片，确认后点击保存即可生效。`
              : `已从电脑导入 ${ids.length} 张图片并填入当前表单，确认后点击保存即可生效。`;
            setNotice(`${summary}${failedDetails ? `部分失败：${failedDetails}` : ""}`);
          } catch (cause) {
            if (mounted.current && scope === scopeEpoch.current && editorSession === editorSessionRef.current) setError(entityError(cause));
          } finally {
            if (importInFlightRef.current === importOp) importInFlightRef.current = null;
            if (mounted.current && scope === scopeEpoch.current) setBusy(false);
          }
        })();
      }}
      onClose={() => setPicking(null)}
    />}
  </section></LibraryThumbnailScope>;
}
