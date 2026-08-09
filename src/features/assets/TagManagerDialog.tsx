import { FormEvent, useEffect, useState } from "react";
import { createAssetTag, deleteAssetTag, listAssetTags, renameAssetTag } from "../../services/tauriClient";
import type { AssetTag } from "../../types/organization";
import { toUserMessage } from "../../i18n/errorMessages";

interface Props { projectId: string; onClose: () => void; onChanged: (tags: AssetTag[]) => void; }

export function TagManagerDialog({ projectId, onClose, onChanged }: Props) {
  const [tags, setTags] = useState<AssetTag[]>([]);
  const [name, setName] = useState("");
  const [editingId, setEditingId] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  async function reload() {
    const next = await listAssetTags(projectId);
    setTags(next);
    onChanged(next);
  }

  useEffect(() => { void reload().catch((value) => setError(toUserMessage(value))); }, [projectId]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!name.trim()) return;
    setBusy(true); setError(undefined);
    try {
      if (editingId) await renameAssetTag(projectId, editingId, name);
      else await createAssetTag(projectId, name);
      setEditingId(undefined); setName(""); await reload();
    } catch (value) { setError(toUserMessage(value)); } finally { setBusy(false); }
  }

  async function remove(tag: AssetTag) {
    if (!window.confirm(`删除标签“${tag.name}”？\n此操作只移除标签，不会删除素材。`)) return;
    setBusy(true); setError(undefined);
    try { await deleteAssetTag(projectId, tag.id); await reload(); }
    catch (value) { setError(toUserMessage(value)); } finally { setBusy(false); }
  }

  return <div className="asset-preview-backdrop" role="presentation" onMouseDown={onClose}>
    <section className="tag-manager-dialog" role="dialog" aria-modal="true" aria-labelledby="tag-manager-title" onMouseDown={(event) => event.stopPropagation()}>
      <div className="section-heading"><div><span className="section-label">素材整理</span><h2 id="tag-manager-title">管理标签</h2></div><button type="button" className="quiet-button" onClick={onClose}>关闭</button></div>
      <form className="tag-manager-form" onSubmit={(event) => void submit(event)}>
        <label><span>{editingId ? "标签新名称" : "新建标签"}</span><input value={name} maxLength={32} placeholder="例如：人物" onChange={(event) => setName(event.target.value)} /></label>
        <button type="submit" disabled={busy || !name.trim()}>{editingId ? "保存名称" : "创建标签"}</button>
        {editingId && <button type="button" className="quiet-button" onClick={() => { setEditingId(undefined); setName(""); }}>取消编辑</button>}
      </form>
      <ul className="tag-manager-list">
        {tags.map((tag) => <li key={tag.id}><span className="asset-tag-chip">{tag.name}</span><span><button type="button" className="quiet-button" onClick={() => { setEditingId(tag.id); setName(tag.name); }}>重命名</button><button type="button" className="quiet-button" onClick={() => void remove(tag)}>删除</button></span></li>)}
        {!tags.length && <li className="empty-state">当前项目还没有标签。</li>}
      </ul>
      {error && <p className="error-message" role="alert">{error}</p>}
    </section>
  </div>;
}
