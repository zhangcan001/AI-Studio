import { useCallback, useEffect, useMemo, useState } from "react";
import { createReferenceAnchor, deleteReferenceAnchor, getAsset, listReferenceAnchors, readAssetImage, readAssetThumbnail, updateReferenceAnchor } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { referenceAnchorKinds, type ReferenceAnchorKind, type ReferenceAnchorRequest, type ReferenceAnchorView } from "../../types/referenceAnchor";
import { ReferenceAnchorEditor } from "./ReferenceAnchorEditor";
import { filterReferenceAnchors, isImageAsset, referenceAnchorKindLabels } from "./referenceAnchorState";

interface Props {
  projectId: string;
  selectedAssets: AssetView[];
  onClearSelection?: () => void;
}

function AnchorThumbnail({ projectId, assetId, asset }: { projectId: string; assetId?: string; asset?: AssetView | null }) {
  const [resolvedAsset, setResolvedAsset] = useState<AssetView>();
  const [url, setUrl] = useState<string>();
  const displayAsset = asset ?? resolvedAsset;

  useEffect(() => {
    let active = true;
    if (asset || !assetId) {
      setResolvedAsset(undefined);
      return () => undefined;
    }
    void getAsset(projectId, assetId)
      .then((next) => { if (active) setResolvedAsset(next); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [asset, assetId, projectId]);

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    if (!displayAsset || !isImageAsset(displayAsset)) return () => undefined;
    void (displayAsset.thumbnailAvailable ? readAssetThumbnail(projectId, displayAsset.id).catch(() => readAssetImage(projectId, displayAsset.id)) : readAssetImage(projectId, displayAsset.id))
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: displayAsset.mimeType }));
        setUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [displayAsset, projectId]);
  if (!displayAsset || !isImageAsset(displayAsset)) return <span className="reference-anchor-thumbnail-placeholder">暂无预览</span>;
  return url ? <img src={url} alt={displayAsset.name} /> : <span className="reference-anchor-thumbnail-placeholder">加载预览…</span>;
}

export function ReferenceAnchorPanel({ projectId, selectedAssets, onClearSelection }: Props) {
  const [anchors, setAnchors] = useState<ReferenceAnchorView[]>([]);
  const [kind, setKind] = useState<ReferenceAnchorKind | "ALL">("ALL");
  const [keyword, setKeyword] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [editorAnchor, setEditorAnchor] = useState<ReferenceAnchorView>();
  const [creating, setCreating] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      setAnchors(await listReferenceAnchors(projectId));
    } catch (value: unknown) {
      setError(value instanceof Error ? value.message : "参考锚点加载失败。");
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { void reload(); }, [reload]);

  const filteredAnchors = useMemo(() => filterReferenceAnchors(anchors, kind, keyword), [anchors, kind, keyword]);
  const selectedImageAssets = useMemo(() => selectedAssets.filter(isImageAsset).slice(0, 20), [selectedAssets]);

  function openCreate() {
    setEditorAnchor(undefined);
    setCreating(true);
  }

  async function saveAnchor(request: ReferenceAnchorRequest) {
    setBusy(true);
    try {
      const saved = editorAnchor
        ? await updateReferenceAnchor({ ...request, projectId, anchorId: editorAnchor.id })
        : await createReferenceAnchor({ ...request, projectId });
      setAnchors((current) => editorAnchor ? current.map((anchor) => anchor.id === saved.id ? saved : anchor) : [saved, ...current]);
      setEditorAnchor(undefined);
      setCreating(false);
      onClearSelection?.();
    } finally {
      setBusy(false);
    }
  }

  async function removeAnchor(anchor: ReferenceAnchorView) {
    if (!window.confirm(`确定删除参考锚点“${anchor.name}”吗？素材本身不会被删除。`)) return;
    setBusy(true);
    setError(undefined);
    try {
      await deleteReferenceAnchor(projectId, anchor.id);
      setAnchors((current) => current.filter((item) => item.id !== anchor.id));
      if (editorAnchor?.id === anchor.id) setEditorAnchor(undefined);
    } catch (value: unknown) {
      setError(value instanceof Error ? value.message : "删除参考锚点失败。");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="reference-anchor-panel" aria-label="参考锚点库">
      <div className="reference-anchor-panel-heading">
        <div>
          <span className="section-label">复用组织</span>
          <h3>参考锚点</h3>
          <p className="section-description">把现有图片素材组织成角色、场景、道具或风格参考集。</p>
        </div>
        <button type="button" className="primary-action" onClick={openCreate} disabled={!selectedImageAssets.length || selectedAssets.length > 20}>
          创建参考锚点
        </button>
      </div>
      <div className="reference-anchor-selection-note">
        {selectedAssets.length ? `已选择 ${selectedAssets.length} 项，${selectedImageAssets.length} 张图片可加入锚点。` : "请先在上方资产库开启“批量选择”并选择图片。"}
        {selectedAssets.length > 20 && <span>一次最多保存 20 张。</span>}
      </div>
      <div className="reference-anchor-controls">
        <label>
          <span>搜索锚点</span>
          <input value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder="按名称或说明搜索" />
        </label>
        <div className="filter-row" aria-label="参考锚点类型">
          <button type="button" className={kind === "ALL" ? "filter-button filter-button-active" : "filter-button"} onClick={() => setKind("ALL")}>全部</button>
          {referenceAnchorKinds.map((value) => (
            <button key={value} type="button" className={kind === value ? "filter-button filter-button-active" : "filter-button"} onClick={() => setKind(value)}>{referenceAnchorKindLabels[value]}</button>
          ))}
        </div>
        <button type="button" className="quiet-button" onClick={() => void reload()} disabled={loading || busy}>{loading ? "正在刷新…" : "刷新"}</button>
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      {loading && !anchors.length && <p className="empty-state">正在加载参考锚点…</p>}
      {!loading && !filteredAnchors.length && <p className="empty-state">当前没有符合条件的参考锚点。</p>}
      <div className="reference-anchor-card-grid">
        {filteredAnchors.map((anchor) => {
          const primary = anchor.assets.find((item) => item.assetId === anchor.primaryAssetId) ?? anchor.assets[0];
          return (
            <article className="reference-anchor-card" key={anchor.id}>
              <div className="reference-anchor-card-thumbnail"><AnchorThumbnail projectId={projectId} assetId={primary?.assetId} asset={primary?.asset} /></div>
              <div className="reference-anchor-card-copy">
                <div className="reference-anchor-card-title"><strong>{anchor.name}</strong><span>{referenceAnchorKindLabels[anchor.kind]}</span></div>
                <small>{anchor.assets.length} 张图片 · {anchor.usable ? "可套用" : "不可套用"}</small>
                {anchor.description && <p>{anchor.description}</p>}
              </div>
              <div className="reference-anchor-card-actions">
                <button type="button" onClick={() => { setEditorAnchor(anchor); setCreating(false); }} disabled={busy}>编辑</button>
                <button type="button" className="danger-button" onClick={() => void removeAnchor(anchor)} disabled={busy}>删除</button>
              </div>
            </article>
          );
        })}
      </div>
      {(creating || editorAnchor) && (
        <ReferenceAnchorEditor
          key={editorAnchor?.id ?? "new"}
          anchor={editorAnchor}
          selectedAssets={selectedImageAssets}
          onSave={saveAnchor}
          onCancel={() => { setCreating(false); setEditorAnchor(undefined); }}
          busy={busy}
        />
      )}
    </section>
  );
}
