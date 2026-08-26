import { useEffect, useMemo, useState } from "react";
import { readAssetThumbnail } from "../../services/tauriClient";
import type {
  ConsistencyProfileView,
  ProfileType,
  ReferenceSetDetailView,
  ReferenceSetDraft,
  ReferenceSetItemInput,
  ReferenceSetItemView,
  ReferenceSetPurpose,
  ReferenceSetView,
} from "../../types/consistency";
import { MAX_REFERENCE_SET_ITEMS } from "../../types/consistency";
import { AssetPickerDialog } from "../studio/AssetPickerDialog";
import { toUserMessage } from "../../i18n/errorMessages";

const purposeLabels: Record<ReferenceSetPurpose, string> = {
  CHARACTER: "角色",
  COSTUME: "服装",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
  SHOT: "镜头",
};

const roleShortcuts = [
  "FACE",
  "FULL_BODY",
  "TURNAROUND",
  "EXPRESSION",
  "ACTION",
  "ENVIRONMENT",
  "DETAIL",
] as const;

interface Props {
  projectId: string;
  referenceSet?: ReferenceSetView;
  detail?: ReferenceSetDetailView;
  ownerProfiles?: ConsistencyProfileView[];
  onSave: (draft: ReferenceSetDraft) => Promise<void>;
  onCancel?: () => void;
  onDelete?: () => Promise<void>;
  onDirtyChange?: (dirty: boolean) => void;
  busy?: boolean;
  deleteBlocked?: boolean;
  error?: string;
}

function ownerTypeForPurpose(purpose: ReferenceSetPurpose): ProfileType | undefined {
  if (purpose === "CHARACTER" || purpose === "COSTUME") return "CHARACTER";
  if (purpose === "SCENE") return "SCENE";
  if (purpose === "PROP") return "PROP";
  if (purpose === "STYLE") return "STYLE";
  return undefined;
}

function initialItems(detail?: ReferenceSetDetailView): ReferenceSetItemView[] {
  return [...(detail?.items ?? [])].sort((left, right) => left.ordinal - right.ordinal);
}

export function normalizeReferenceSetItems(items: ReferenceSetItemView[]): ReferenceSetItemInput[] {
  return items.slice(0, MAX_REFERENCE_SET_ITEMS).map((item, ordinal) => ({
    assetId: item.assetId,
    ordinal,
    role: item.role?.trim() || null,
    isPrimary: item.isPrimary,
  }));
}

function ReferenceItemThumbnail({ projectId, item }: { projectId: string; item: ReferenceSetItemView }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    if (!item.thumbnailAvailable) return () => undefined;
    let active = true;
    let objectUrl: string | undefined;
    void readAssetThumbnail(projectId, item.assetId)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [item.assetId, item.thumbnailAvailable, projectId]);
  if (url) return <img src={url} alt={item.assetName ?? item.assetId} style={{ width: "100%", height: "100%", objectFit: "cover" }} />;
  return <span style={{ color: "var(--studio-text-muted, #6b7280)", fontSize: "0.7rem" }}>暂无预览</span>;
}

export function ReferenceSetEditor({
  projectId,
  referenceSet,
  detail,
  ownerProfiles = [],
  onSave,
  onCancel,
  onDelete,
  onDirtyChange,
  busy = false,
  deleteBlocked = false,
  error,
}: Props) {
  const base = detail?.referenceSet ?? referenceSet;
  const [name, setName] = useState(base?.name ?? "");
  const [purpose, setPurpose] = useState<ReferenceSetPurpose>(base?.purpose ?? "CHARACTER");
  const [description, setDescription] = useState(base?.description ?? "");
  const [ownerProfileId, setOwnerProfileId] = useState(base?.ownerProfileId ?? "");
  const [items, setItems] = useState<ReferenceSetItemView[]>(() => initialItems(detail));
  const [pickerOpen, setPickerOpen] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string>();

  const ownerProfileType = ownerTypeForPurpose(purpose);
  const candidates = useMemo(
    () => ownerProfileType ? ownerProfiles.filter((profile) => profile.profileType === ownerProfileType) : [],
    [ownerProfileType, ownerProfiles],
  );
  const savingNow = busy || saving;

  function markDirty() {
    setDirty((current) => {
      if (!current) onDirtyChange?.(true);
      return true;
    });
  }

  function changePurpose(next: ReferenceSetPurpose) {
    setPurpose(next);
    const nextOwnerType = ownerTypeForPurpose(next);
    if (!nextOwnerType || nextOwnerType !== ownerProfileType) setOwnerProfileId("");
    markDirty();
  }

  function changeItems(next: ReferenceSetItemView[]) {
    setItems(next.map((item, ordinal) => ({ ...item, ordinal })));
    markDirty();
  }

  function confirmPickedAssetIds(assetIds: string[]) {
    const existing = new Map(items.map((item) => [item.assetId, item]));
    const next = assetIds.slice(0, MAX_REFERENCE_SET_ITEMS).map((assetId, ordinal) => existing.get(assetId) ?? ({
      assetId,
      ordinal,
      role: null,
      isPrimary: false,
    }));
    changeItems(next);
    setPickerOpen(false);
  }

  function moveItem(index: number, delta: -1 | 1) {
    const target = index + delta;
    if (target < 0 || target >= items.length) return;
    const next = [...items];
    [next[index], next[target]] = [next[target], next[index]];
    changeItems(next);
  }

  function removeItem(assetId: string) {
    changeItems(items.filter((item) => item.assetId !== assetId));
  }

  function setPrimary(assetId: string) {
    changeItems(items.map((item) => ({ ...item, isPrimary: item.assetId === assetId ? !item.isPrimary : false })));
  }

  function setRole(assetId: string, role: string) {
    changeItems(items.map((item) => item.assetId === assetId ? { ...item, role } : item));
  }

  async function save() {
    if (!name.trim()) {
      setFormError("请输入参考集名称。");
      return;
    }
    if (items.length > MAX_REFERENCE_SET_ITEMS) {
      setFormError(`参考集最多包含 ${MAX_REFERENCE_SET_ITEMS} 张图片。`);
      return;
    }
    setFormError(undefined);
    setSaving(true);
    try {
      await onSave({
        name: name.trim(),
        purpose,
        description: description.trim(),
        ownerProfileType: ownerProfileType ?? null,
        ownerProfileId: ownerProfileId || null,
        items: normalizeReferenceSetItems(items),
      });
      setDirty(false);
      onDirtyChange?.(false);
    } catch (value: unknown) {
      setFormError(toUserMessage(value));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="reference-set-editor" aria-label={base ? "编辑参考集" : "新建参考集"} style={{ display: "grid", gap: 14, minWidth: 0 }}>
      <div className="section-heading" style={{ alignItems: "flex-start", marginBottom: 0 }}>
        <div>
          <span className="section-label">{base ? "ReferenceSet 编辑" : "新建 ReferenceSet"}</span>
          <h3>{base ? "编辑参考集" : "新建参考集"}</h3>
          <p className="section-description">按稳定顺序组织最多 {MAX_REFERENCE_SET_ITEMS} 张图片，供 Profile 和镜头上下文复用。</p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
          {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={savingNow}>取消</button>}
          {base && onDelete && <button type="button" className="danger-button" onClick={() => void onDelete()} disabled={savingNow || deleteBlocked}>删除参考集</button>}
        </div>
      </div>

      {dirty && <p className="disabled-note" role="status">有未保存的修改，切换参考集前会再次确认。</p>}
      {(error || formError) && <p className="error-message" role="alert">{formError ?? error}</p>}

      <div style={{ display: "grid", gap: 10 }}>
        <label className="field-control"><span>名称</span><input value={name} maxLength={120} onChange={(event) => { setName(event.target.value); markDirty(); setFormError(undefined); }} placeholder="例如：主角正面与全身" disabled={savingNow} /></label>
        <div style={{ display: "grid", gridTemplateColumns: "minmax(160px, .7fr) minmax(220px, 1fr)", gap: 10 }}>
          <label className="field-control"><span>用途</span><select value={purpose} onChange={(event) => changePurpose(event.target.value as ReferenceSetPurpose)} disabled={savingNow}>{Object.entries(purposeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
          <label className="field-control"><span>所有者 Profile</span><select value={ownerProfileId} onChange={(event) => { setOwnerProfileId(event.target.value); markDirty(); }} disabled={savingNow || !ownerProfileType}><option value="">{ownerProfileType ? "不设置" : "镜头用途不支持所有者"}</option>{candidates.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}</select></label>
        </div>
        <label className="field-control"><span>说明</span><textarea value={description} maxLength={1000} onChange={(event) => { setDescription(event.target.value); markDirty(); }} rows={3} placeholder="可选：说明这组参考图的使用边界。" disabled={savingNow} /></label>
      </div>

      <section className="reference-set-items" aria-label="参考集图片" style={{ display: "grid", gap: 9, paddingTop: 8, borderTop: "1px solid var(--studio-border, rgba(255,255,255,.08))" }}>
        <div className="section-heading" style={{ marginBottom: 0 }}>
          <div>
            <strong>有序参考图（{items.length}/{MAX_REFERENCE_SET_ITEMS}）</strong>
            <p className="section-description">删除或重排后，保存会自动重建 ordinal 0..N-1；最多只能有一个主图。</p>
          </div>
          <button type="button" onClick={() => setPickerOpen(true)} disabled={savingNow || items.length >= MAX_REFERENCE_SET_ITEMS}>添加图片</button>
        </div>
        {!items.length && <p className="empty-state">暂无参考图。点击“添加图片”复用素材库中的图片选择器。</p>}
        {items.map((item, index) => (
          <article key={item.assetId} className="reference-set-item-row" style={{ display: "grid", gridTemplateColumns: "44px minmax(0, 1fr) auto", gap: 10, alignItems: "center", padding: 9, border: "1px solid var(--studio-border, rgba(255,255,255,.08))", borderRadius: 8 }}>
            <div style={{ display: "grid", placeItems: "center", width: 44, height: 44, overflow: "hidden", borderRadius: 6, background: "var(--studio-bg, #0b0d10)" }}><ReferenceItemThumbnail projectId={projectId} item={item} /></div>
            <div style={{ display: "grid", gap: 5, minWidth: 0 }}>
              <strong style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{item.assetName ?? item.assetId}</strong>
              <small style={{ color: "var(--studio-text-secondary, #9ca3af)" }}>顺序 {index + 1} · {item.width ?? "--"} × {item.height ?? "--"}{item.isPrimary ? " · 主图" : ""}</small>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 5 }}>
                {roleShortcuts.map((role) => <button key={role} type="button" className={item.role === role ? "filter-button filter-button-active" : "quiet-button"} onClick={() => setRole(item.assetId, role)} disabled={savingNow}>{role}</button>)}
                <input value={item.role ?? ""} onChange={(event) => setRole(item.assetId, event.target.value)} placeholder="自定义 role" aria-label={`${item.assetName ?? item.assetId} 的 role`} disabled={savingNow} style={{ width: 140, minHeight: 30 }} />
              </div>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 5, alignItems: "stretch" }}>
              <button type="button" className="quiet-button" onClick={() => moveItem(index, -1)} disabled={savingNow || index === 0}>上移</button>
              <button type="button" className="quiet-button" onClick={() => moveItem(index, 1)} disabled={savingNow || index === items.length - 1}>下移</button>
              <button type="button" className={item.isPrimary ? "filter-button filter-button-active" : "quiet-button"} onClick={() => setPrimary(item.assetId)} disabled={savingNow}>{item.isPrimary ? "取消主图" : "设为主图"}</button>
              <button type="button" className="danger-button" onClick={() => removeItem(item.assetId)} disabled={savingNow}>删除</button>
            </div>
          </article>
        ))}
      </section>

      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", flexWrap: "wrap" }}>
        <button type="button" className="primary-action" onClick={() => void save()} disabled={savingNow || !name.trim()}>{saving ? "正在保存…" : base ? "保存参考集" : "创建参考集"}</button>
        {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={savingNow}>取消</button>}
      </div>

      {pickerOpen && <AssetPickerDialog projectId={projectId} kind="image" multiple maxItems={MAX_REFERENCE_SET_ITEMS} selectedIds={items.map((item) => item.assetId)} onCancel={() => setPickerOpen(false)} onConfirm={confirmPickedAssetIds} />}
    </section>
  );
}

export { ownerTypeForPurpose, purposeLabels, roleShortcuts };
