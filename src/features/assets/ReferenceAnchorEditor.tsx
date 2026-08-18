import { useMemo, useState } from "react";
import type { AssetView } from "../../types/asset";
import { referenceAnchorKinds, type ReferenceAnchorAssetView, type ReferenceAnchorKind, type ReferenceAnchorRequest, type ReferenceAnchorView } from "../../types/referenceAnchor";
import { referenceAnchorKindLabels, appendUniqueReferenceAssets, orderedReferenceAnchorAssets } from "./referenceAnchorState";

interface Props {
  anchor?: ReferenceAnchorView;
  selectedAssets: AssetView[];
  onSave: (request: ReferenceAnchorRequest) => Promise<void>;
  onCancel: () => void;
  busy?: boolean;
}

function seedAssets(anchor: ReferenceAnchorView | undefined, selectedAssets: AssetView[]): ReferenceAnchorAssetView[] {
  if (anchor) return orderedReferenceAnchorAssets(anchor.assets);
  return selectedAssets.map((asset, ordinal) => ({ assetId: asset.id, ordinal, asset }));
}

export function ReferenceAnchorEditor({ anchor, selectedAssets, onSave, onCancel, busy = false }: Props) {
  const initialAssets = useMemo(() => seedAssets(anchor, selectedAssets), [anchor, selectedAssets]);
  const [kind, setKind] = useState<ReferenceAnchorKind>(anchor?.kind ?? "CHARACTER");
  const [name, setName] = useState(anchor?.name ?? "");
  const [description, setDescription] = useState(anchor?.description ?? "");
  const [assets, setAssets] = useState<ReferenceAnchorAssetView[]>(initialAssets);
  const [error, setError] = useState<string>();

  function moveAsset(index: number, delta: -1 | 1) {
    const target = index + delta;
    if (target < 0 || target >= assets.length) return;
    setAssets((current) => {
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next.map((item, ordinal) => ({ ...item, ordinal }));
    });
  }

  function moveAssetToFront(index: number) {
    if (index === 0) return;
    setAssets((current) => [current[index], ...current.filter((_, itemIndex) => itemIndex !== index)].map((item, ordinal) => ({ ...item, ordinal })));
  }

  function removeAsset(assetId: string) {
    setAssets((current) => current.filter((item) => item.assetId !== assetId).map((item, ordinal) => ({ ...item, ordinal })));
  }

  function addSelectedAssets() {
    setAssets((current) => appendUniqueReferenceAssets(current, selectedAssets));
  }

  async function save() {
    const normalizedName = name.trim();
    if (!normalizedName) {
      setError("请输入参考锚点名称。");
      return;
    }
    if (!anchor && !assets.length) {
      setError("创建参考锚点至少需要一张图片。");
      return;
    }
    setError(undefined);
    try {
      await onSave({
        projectId: anchor?.projectId ?? "",
        kind,
        name: normalizedName,
        description: description.trim(),
        assetIds: assets.map((item) => item.assetId),
      });
    } catch (value: unknown) {
      setError(value instanceof Error ? value.message : "保存参考锚点失败。");
    }
  }

  return (
    <section className="reference-anchor-editor" aria-label={anchor ? "编辑参考锚点" : "创建参考锚点"}>
      <div className="reference-anchor-editor-heading">
        <div>
          <span className="section-label">参考锚点</span>
          <h3>{anchor ? "编辑参考锚点" : "创建参考锚点"}</h3>
        </div>
        <button type="button" className="quiet-button" onClick={onCancel} disabled={busy}>关闭</button>
      </div>
      <div className="reference-anchor-editor-fields">
        <label>
          <span>类型</span>
          <select value={kind} onChange={(event) => setKind(event.target.value as ReferenceAnchorKind)} disabled={busy}>
            {referenceAnchorKinds.map((value) => <option key={value} value={value}>{referenceAnchorKindLabels[value]}</option>)}
          </select>
        </label>
        <label>
          <span>名称</span>
          <input value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="例如：地藏菩萨" disabled={busy} />
        </label>
        <label className="reference-anchor-description-field">
          <span>说明</span>
          <textarea value={description} maxLength={500} onChange={(event) => setDescription(event.target.value)} placeholder="可选" rows={2} disabled={busy} />
        </label>
      </div>
      <div className="reference-anchor-editor-assets">
        <div className="reference-anchor-editor-assets-heading">
          <strong>有序参考图（{assets.length}/20）</strong>
          <button type="button" onClick={addSelectedAssets} disabled={busy || !selectedAssets.length || assets.length >= 20}>添加已选择图片</button>
        </div>
        {!assets.length && <p className="empty-state">暂无参考图 · 当前锚点不可套用</p>}
        {assets.map((item, index) => (
          <div className="reference-anchor-editor-asset" key={item.assetId}>
            <span className="reference-anchor-editor-ordinal">{index + 1}</span>
            <span className="reference-anchor-editor-asset-name">{item.asset?.name ?? item.assetId}</span>
            {index === 0 && <span className="reference-anchor-primary-badge">主参考</span>}
            <button type="button" className="quiet-button" onClick={() => moveAsset(index, -1)} disabled={busy || index === 0}>上移</button>
            <button type="button" className="quiet-button" onClick={() => moveAsset(index, 1)} disabled={busy || index === assets.length - 1}>下移</button>
            <button type="button" className="quiet-button" onClick={() => moveAssetToFront(index)} disabled={busy || index === 0} aria-label="设为主参考">设为主参考</button>
            <button type="button" className="danger-button" onClick={() => removeAsset(item.assetId)} disabled={busy}>移除</button>
          </div>
        ))}
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      <div className="reference-anchor-editor-actions">
        <button type="button" className="primary-action" onClick={() => void save()} disabled={busy}>{busy ? "正在保存…" : "保存参考锚点"}</button>
        <button type="button" className="quiet-button" onClick={onCancel} disabled={busy}>取消</button>
      </div>
    </section>
  );
}
