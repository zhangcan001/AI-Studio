import { useEffect, useState } from "react";
import { assignAssetTag, createAssetTag, getAsset, getAssetMediaUrl, getAssetVideoPrompt, readAssetImage, readAssetThumbnail, removeAssetTag, setAssetFavorite, setAssetVideoPrompt } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { AssetTag } from "../../types/organization";
import { assetDisplayName, assetTypeLabel, formatDateTime, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  asset: AssetView;
  onClose: () => void;
  onUseInStudio?: (asset: AssetView) => void;
  onOpenTask?: (taskId: string) => void;
  allTags?: AssetTag[];
  onOrganizationChanged?: (asset: AssetView) => void;
  onRequestDelete?: (asset: AssetView) => void;
}

export function AssetPreview({ projectId, asset, onClose, onUseInStudio, onOpenTask, allTags = [], onOrganizationChanged, onRequestDelete }: Props) {
  const [url, setUrl] = useState<string>();
  const [posterUrl, setPosterUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const [selectedTagId, setSelectedTagId] = useState("");
  const [newTagName, setNewTagName] = useState("");
  const [organizationBusy, setOrganizationBusy] = useState(false);
  const [videoPrompt, setVideoPrompt] = useState("");
  const [videoPromptBusy, setVideoPromptBusy] = useState(false);
  const [videoPromptNotice, setVideoPromptNotice] = useState<string>();

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    let posterObjectUrl: string | undefined;
    setUrl(undefined);
    setPosterUrl(undefined);
    setError(undefined);
    const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
    const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
    if (isVideo || isAudio) {
      setUrl(getAssetMediaUrl(projectId, asset.id, isVideo ? "video" : "audio"));
      setError(undefined);
      if (asset.thumbnailAvailable) {
        void readAssetThumbnail(projectId, asset.id)
          .then((bytes) => {
            if (!active) return;
            posterObjectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
            setPosterUrl(posterObjectUrl);
          })
          .catch(() => undefined);
      }
      return () => {
        active = false;
        if (posterObjectUrl) URL.revokeObjectURL(posterObjectUrl);
      };
    }
    void readAssetImage(projectId, asset.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
        setUrl(objectUrl);
      })
      .catch(() => {
        if (active) setError("暂无预览，请稍后重试。");
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      if (posterObjectUrl) URL.revokeObjectURL(posterObjectUrl);
    };
  }, [asset.assetType, asset.category, asset.id, asset.mimeType, asset.thumbnailAvailable, projectId]);

  const isImage = asset.assetType === "image" || asset.category === "source_image" || asset.category === "generated_image";

  useEffect(() => {
    let active = true;
    setVideoPrompt("");
    setVideoPromptNotice(undefined);
    if (!isImage) return () => { active = false; };
    void getAssetVideoPrompt(projectId, asset.id)
      .then((record) => {
        if (active) setVideoPrompt(record?.promptText ?? "");
      })
      .catch(() => {
        if (active) setVideoPromptNotice("视频提示词读取失败，请稍后重试。");
      });
    return () => { active = false; };
  }, [asset.id, isImage, projectId]);

  const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
  const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
  const videoPromptBytes = new TextEncoder().encode(videoPrompt).byteLength;
  const displayName = assetDisplayName(asset);
  const displayOriginalName = assetDisplayName(asset, asset.originalName);

  async function refreshOrganization() {
    const refreshed = await getAsset(projectId, asset.id);
    onOrganizationChanged?.(refreshed);
  }

  async function updateFavorite() {
    setOrganizationBusy(true); setError(undefined);
    try { await setAssetFavorite(projectId, asset.id, !asset.isFavorite); await refreshOrganization(); }
    catch { setError("收藏状态更新失败，请稍后重试。"); } finally { setOrganizationBusy(false); }
  }

  async function addExistingTag() {
    if (!selectedTagId) return;
    setOrganizationBusy(true); setError(undefined);
    try { await assignAssetTag(projectId, asset.id, selectedTagId); setSelectedTagId(""); await refreshOrganization(); }
    catch { setError("标签添加失败，请检查标签数量后重试。"); } finally { setOrganizationBusy(false); }
  }

  async function createAndAddTag() {
    if (!newTagName.trim()) return;
    setOrganizationBusy(true); setError(undefined);
    try { const tag = await createAssetTag(projectId, newTagName); await assignAssetTag(projectId, asset.id, tag.id); setNewTagName(""); await refreshOrganization(); }
    catch { setError("标签创建失败，名称可能已存在。"); } finally { setOrganizationBusy(false); }
  }

  async function removeTag(tagId: string) {
    setOrganizationBusy(true); setError(undefined);
    try { await removeAssetTag(projectId, asset.id, tagId); await refreshOrganization(); }
    catch { setError("标签移除失败，请稍后重试。"); } finally { setOrganizationBusy(false); }
  }

  async function saveVideoPrompt() {
    setVideoPromptBusy(true);
    setVideoPromptNotice(undefined);
    try {
      await setAssetVideoPrompt(projectId, asset.id, videoPrompt);
      setVideoPromptNotice("已配置");
    } catch {
      setVideoPromptNotice("保存失败：请输入非空提示词，且不超过 64 KiB。");
    } finally {
      setVideoPromptBusy(false);
    }
  }

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <div className="asset-preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="asset-preview-panel"
        role="dialog"
        aria-modal="true"
        aria-label={`${displayName} 预览`}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="section-heading">
          <div>
            <span className="section-label">资产预览</span>
            <h2>{displayName}</h2>
          </div>
          <div className="asset-preview-actions">
            <button type="button" className="quiet-button" aria-pressed={asset.isFavorite} aria-label={asset.isFavorite ? "取消收藏素材" : "收藏素材"} onClick={() => void updateFavorite()} disabled={organizationBusy}>{asset.isFavorite ? "★ 取消收藏" : "☆ 收藏"}</button>
            {onUseInStudio && <button type="button" onClick={() => onUseInStudio(asset)}>用于创作</button>}
            {asset.sourceTaskId && onOpenTask && (
              <button type="button" className="quiet-button" onClick={() => onOpenTask(asset.sourceTaskId!)}>
                查看生成任务
              </button>
            )}
            {onRequestDelete && <button type="button" className="danger-button" onClick={() => onRequestDelete(asset)} disabled={organizationBusy}>删除素材</button>}
            <button type="button" className="quiet-button" onClick={onClose} aria-label="关闭预览">
              关闭
            </button>
          </div>
        </div>
        <div className="asset-preview-image">
          {isVideo && url ? (
            <video src={url} poster={posterUrl} controls preload="metadata" playsInline aria-label={displayName} />
          ) : isAudio && url ? (
            <audio src={url} controls preload="metadata" aria-label={displayName} />
          ) : url ? <img src={url} alt={displayName} /> : <p>{error ?? "正在加载预览..."}</p>}
        </div>
        <p className="asset-preview-meta">
          {assetTypeLabel(asset)} · {displayOriginalName} · {isVideo || isAudio ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`} · {formatFileSize(asset.fileSize)} · {formatDateTime(asset.createdAt)}
        </p>
        {isImage && (
          <section className="asset-video-prompt-panel" aria-label="视频提示词">
            <div className="asset-video-prompt-heading">
              <div>
                <strong>视频提示词</strong>
                <small>资产可直接用于 MiniMax H3 批量视频；允许保留内部换行。</small>
              </div>
              <span className={videoPrompt.trim() ? "asset-prompt-status asset-prompt-status-ready" : "asset-prompt-status"}>
                {videoPrompt.trim() ? "已配置" : "未配置"}
              </span>
            </div>
            <textarea
              value={videoPrompt}
              maxLength={64 * 1024}
              rows={4}
              aria-label="视频提示词内容"
              placeholder="描述这张图片要如何运动或变化……"
              onChange={(event) => {
                const next = event.target.value;
                if (new TextEncoder().encode(next).byteLength <= 64 * 1024) setVideoPrompt(next);
                else setVideoPromptNotice("提示词不能超过 64 KiB。");
              }}
              disabled={videoPromptBusy}
            />
            <div className="asset-video-prompt-actions">
              <small>{videoPromptBytes.toLocaleString()} / 65,536 字节</small>
              <button type="button" onClick={() => void saveVideoPrompt()} disabled={videoPromptBusy || !videoPrompt.trim() || videoPromptBytes > 64 * 1024}>
                {videoPromptBusy ? "正在保存..." : "保存提示词"}
              </button>
            </div>
            {videoPromptNotice && <p className="disabled-note" role="status">{videoPromptNotice}</p>}
          </section>
        )}
        <section className="asset-preview-tags" aria-label="素材标签">
          <strong>标签</strong>
          <div className="asset-preview-tag-list">
            {asset.tags.map((tag) => <button key={tag.id} type="button" className="asset-tag-chip" aria-label={`移除标签${tag.name}`} onClick={() => void removeTag(tag.id)} disabled={organizationBusy}>{tag.name} ×</button>)}
            {!asset.tags.length && <span>暂未添加标签</span>}
          </div>
          <div className="asset-preview-tag-actions">
            <select aria-label="选择已有标签" value={selectedTagId} onChange={(event) => setSelectedTagId(event.target.value)} disabled={organizationBusy}>
              <option value="">选择已有标签</option>
              {allTags.filter((tag) => !asset.tags.some((assigned) => assigned.id === tag.id)).map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}
            </select>
            <button type="button" onClick={() => void addExistingTag()} disabled={organizationBusy || !selectedTagId}>添加标签</button>
            <input aria-label="新标签名称" value={newTagName} maxLength={32} placeholder="新标签名称" onChange={(event) => setNewTagName(event.target.value)} disabled={organizationBusy} />
            <button type="button" className="quiet-button" onClick={() => void createAndAddTag()} disabled={organizationBusy || !newTagName.trim()}>新建并添加</button>
          </div>
        </section>
      </section>
    </div>
  );
}
