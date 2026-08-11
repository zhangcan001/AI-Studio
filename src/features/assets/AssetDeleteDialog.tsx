import { useEffect, useState } from "react";
import { deleteAssets, inspectAssetDeletion } from "../../services/tauriClient";
import type { AssetDeleteInspection, AssetDeleteResult, AssetView } from "../../types/asset";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { formatFileSize } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  assets: AssetView[];
  onClose: () => void;
  onDeleted: (assetIds: string[], result: AssetDeleteResult) => void;
}

export function AssetDeleteDialog({ projectId, assets, onClose, onDeleted }: Props) {
  const [inspection, setInspection] = useState<AssetDeleteInspection>();
  const [error, setError] = useState<unknown>();
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    let active = true;
    void inspectAssetDeletion(projectId, assets.map((asset) => asset.id))
      .then((next) => {
        if (active) setInspection(next);
      })
      .catch((nextError: unknown) => {
        if (active) setError(nextError);
      });
    return () => {
      active = false;
    };
  }, [assets, projectId]);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && !deleting) onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [deleting, onClose]);

  const blocked = inspection?.blocked.length ?? assets.length;
  const ready = Boolean(inspection && inspection.blocked.length === 0);

  async function confirmDelete() {
    if (!ready) return;
    setDeleting(true);
    setError(undefined);
    try {
      const result = await deleteAssets(projectId, assets.map((asset) => asset.id));
      onDeleted(assets.map((asset) => asset.id), result);
    } catch (deleteError: unknown) {
      setError(deleteError);
    } finally {
      setDeleting(false);
    }
  }

  return (
    <div className="asset-delete-backdrop" role="presentation" onMouseDown={() => !deleting && onClose()}>
      <section
        className="asset-delete-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="asset-delete-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="section-heading">
          <div>
            <span className="section-label">资产库</span>
            <h2 id="asset-delete-title">删除素材</h2>
          </div>
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>关闭</button>
        </div>
        <p className="asset-delete-warning">此操作会删除 AI Studio 项目中的素材文件，无法撤销。</p>
        {error !== undefined && <UiErrorNotice error={error} />}
        {!inspection && !error && <p className="disabled-note">正在检查素材引用……</p>}
        {inspection && (
          <>
            <p className="asset-delete-summary">可删除 {inspection.deletable.length} 项，不可删除 {blocked} 项。</p>
            <div className="asset-delete-list">
              {inspection.items.map((item) => (
                <article key={item.assetId} className={item.canDelete ? "asset-delete-item" : "asset-delete-item asset-delete-item-blocked"}>
                  <div>
                    <strong>{item.name}</strong>
                    <span>{item.assetType} · {formatFileSize(item.fileSize)}</span>
                  </div>
                  <div>
                    {item.blockingReasons.map((reason) => <p key={reason} className="asset-delete-reason">{reason}</p>)}
                    {item.warnings.map((warning) => <p key={warning} className="asset-delete-warning-detail">{warning}</p>)}
                    {!item.blockingReasons.length && !item.warnings.length && <span className="asset-delete-ready">可以删除</span>}
                  </div>
                </article>
              ))}
            </div>
            {ready && <p className="asset-delete-confirm-copy">将永久删除 {assets.length} 个素材及其项目存储文件；任务历史不会被删除。</p>}
          </>
        )}
        <div className="asset-delete-actions">
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>取消</button>
          <button type="button" className="danger-button" onClick={() => void confirmDelete()} disabled={!ready || deleting}>
            {deleting ? "正在删除……" : `确认删除${assets.length > 1 ? `（${assets.length}）` : ""}`}
          </button>
        </div>
      </section>
    </div>
  );
}
