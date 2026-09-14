import { useEffect, useState } from "react";
import { assignAssetTag, createAssetTag, getAsset, getAssetMediaUrl, getAssetVideoPrompt, listAssetRelations, listAssetVersions, listGenerationAssetVersionLinks, listGenerationToolUsages, readAssetImage, readAssetThumbnail, removeAssetTag, setAssetFavorite, setAssetVideoPrompt } from "../../services/tauriClient";
import type { AssetRelationView, AssetVersionView, AssetView } from "../../types/asset";
import type { AssetTag } from "../../types/organization";
import type { GenerationAssetVersionView, GenerationToolUsageView } from "../../types/provenance";
import { assetDisplayName, assetTypeLabel, formatDateTime, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";
import { AssetUsagePanel } from "./AssetUsagePanel";

interface Props {
  projectId: string;
  asset: AssetView;
  onClose: () => void;
  onUseInStudio?: (asset: AssetView) => void;
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
  allTags?: AssetTag[];
  onOrganizationChanged?: (asset: AssetView) => void;
  onRequestDelete?: (asset: AssetView) => void;
}

export function AssetPreview({ projectId, asset, onClose, onUseInStudio, onOpenTask, onOpenShot, allTags = [], onOrganizationChanged, onRequestDelete }: Props) {
  const [url, setUrl] = useState<string>();
  const [posterUrl, setPosterUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const [selectedTagId, setSelectedTagId] = useState("");
  const [newTagName, setNewTagName] = useState("");
  const [organizationBusy, setOrganizationBusy] = useState(false);
  const [videoPrompt, setVideoPrompt] = useState("");
  const [videoPromptBusy, setVideoPromptBusy] = useState(false);
  const [videoPromptNotice, setVideoPromptNotice] = useState<string>();
  const [versions, setVersions] = useState<AssetVersionView[]>([]);
  const [relations, setRelations] = useState<AssetRelationView[]>([]);
  const [historyLoading, setHistoryLoading] = useState(true);
  const [historyError, setHistoryError] = useState<string>();
  const [toolUsages, setToolUsages] = useState<GenerationToolUsageView[]>([]);
  const [assetVersionLinks, setAssetVersionLinks] = useState<GenerationAssetVersionView[]>([]);
  const [provenanceLoading, setProvenanceLoading] = useState(true);
  const [provenanceError, setProvenanceError] = useState<string>();

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
    setVersions([]);
    setRelations([]);
    setHistoryLoading(true);
    setHistoryError(undefined);
    void Promise.all([
      listAssetVersions(projectId, asset.id),
      listAssetRelations(projectId, asset.id),
    ])
      .then(([nextVersions, nextRelations]) => {
        if (!active) return;
        setVersions(nextVersions);
        setRelations(nextRelations);
      })
      .catch((value: unknown) => {
        if (active) setHistoryError(toAssetDetailError(value));
      })
      .finally(() => {
        if (active) setHistoryLoading(false);
      });
    return () => { active = false; };
  }, [asset.id, projectId]);

  useEffect(() => {
    let active = true;
    setToolUsages([]);
    setAssetVersionLinks([]);
    setProvenanceError(undefined);
    if (!asset.sourceTaskId) {
      setProvenanceLoading(false);
      return () => { active = false; };
    }
    setProvenanceLoading(true);
    void Promise.all([
      listGenerationToolUsages(projectId, asset.sourceTaskId),
      listGenerationAssetVersionLinks(projectId, asset.sourceTaskId),
    ])
      .then(([nextToolUsages, nextAssetVersionLinks]) => {
        if (!active) return;
        setToolUsages(nextToolUsages);
        setAssetVersionLinks(nextAssetVersionLinks);
      })
      .catch((value: unknown) => {
        if (active) setProvenanceError(toAssetDetailError(value));
      })
      .finally(() => {
        if (active) setProvenanceLoading(false);
      });
    return () => { active = false; };
  }, [asset.sourceTaskId, projectId]);

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
  const currentVersion = versions.reduce<AssetVersionView | undefined>((current, version) => (
    !current || version.versionNumber > current.versionNumber ? version : current
  ), undefined);
  const visibleAssetVersionLinks = versions.length
    ? assetVersionLinks.filter((link) => versions.some((version) => version.id === link.assetVersionId))
    : [];

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
        <section className="asset-detail-metadata" aria-label="素材元数据">
          <div className="asset-detail-section-heading"><strong>元数据</strong><span className="status-pill">项目：{projectId}</span></div>
          <dl className="asset-detail-definition-list">
            <div><dt>名称</dt><dd>{displayName}</dd></div>
            <div><dt>类型</dt><dd>{assetTypeLabel(asset)}</dd></div>
            <div><dt>描述</dt><dd>{"暂无描述"}</dd></div>
            <div><dt>标签</dt><dd>{asset.tags.length ? asset.tags.map((tag) => tag.name).join("、") : "暂无标签"}</dd></div>
            <div><dt>当前版本</dt><dd>{currentVersion ? `v${currentVersion.versionNumber}` : "未建立版本记录"}</dd></div>
            <div><dt>更新时间</dt><dd>{formatDateTime(asset.updatedAt ?? asset.createdAt)}</dd></div>
          </dl>
        </section>
        <section className="asset-detail-section" aria-label="版本历史">
          <div className="asset-detail-section-heading"><div><strong>版本历史</strong><small>历史版本只读，不会覆盖已有素材。</small></div>{currentVersion && <span className="status-pill">当前 v{currentVersion.versionNumber}</span>}</div>
          {historyLoading && <p className="disabled-note" role="status">正在加载版本历史…</p>}
          {historyError && <p className="error-message" role="alert">版本与关系加载失败：{historyError}</p>}
          {!historyLoading && !historyError && !versions.length && <p className="empty-state">暂无版本历史。当前资产仍可作为未建立版本记录的旧资产使用。</p>}
          {!historyLoading && !historyError && versions.length > 0 && (
            <ol className="asset-version-history">
              {[...versions].sort((left, right) => right.versionNumber - left.versionNumber).map((version) => (
                <li key={version.id} className={version.id === currentVersion?.id ? "asset-version-current" : undefined}>
                  <strong>v{version.versionNumber}</strong>
                  {version.id === currentVersion?.id && <span className="status-pill">当前</span>}
                  <small>{formatDateTime(version.createdAt)}</small>
                </li>
              ))}
            </ol>
          )}
        </section>
        <section className="asset-detail-section" aria-label="资产关系">
          <div className="asset-detail-section-heading"><div><strong>资产关系</strong><small>关系使用稳定资产 ID，并保持当前项目隔离。</small></div><span className="status-pill">{relations.length} 条</span></div>
          {!historyLoading && !historyError && !relations.length && <p className="empty-state">暂无资产关系。</p>}
          {!historyLoading && !historyError && relations.length > 0 && (
            <ul className="asset-relation-list">
              {relations.map((relation) => {
                const outgoing = relation.sourceAssetId === asset.id;
                const relatedName = outgoing ? relation.targetAssetName : relation.sourceAssetName;
                const relatedId = outgoing ? relation.targetAssetId : relation.sourceAssetId;
                return <li key={relation.id}><strong>{relationTypeLabel(relation.relationType)}</strong><span>{relatedName || "未命名资产"}</span><small>{relatedId} · {formatDateTime(relation.createdAt)}</small></li>;
              })}
            </ul>
          )}
        </section>
        <section className="asset-detail-section" aria-label="素材来源与溯源">
          <div className="asset-detail-section-heading"><div><strong>来源与溯源</strong><small>只展示现有 Asset、Task 与显式跨模块关系，不推断历史关联。</small></div></div>
          <dl className="asset-detail-definition-list">
            <div><dt>Prompt</dt><dd>{asset.sourceTaskId ? "随生成任务记录" : "未关联 Prompt"}</dd></div>
            <div><dt>Model</dt><dd>未记录</dd></div>
            <div><dt>Generation</dt><dd>{asset.sourceTaskId ? `任务 ${asset.sourceTaskId}` : "本地导入或未关联生成记录"}</dd></div>
            <div><dt>Date</dt><dd>{formatDateTime(asset.createdAt)}</dd></div>
          </dl>
          <section className="asset-detail-section" aria-label="Generation History">
            <div className="asset-detail-section-heading"><div><strong>Generation History</strong><small>跨模块溯源只接受后端已保存的显式关系。</small></div></div>
            {provenanceLoading && <p className="disabled-note" role="status">正在加载生成溯源…</p>}
            {provenanceError && <p className="error-message" role="alert">生成溯源加载失败：{provenanceError}</p>}
            {!provenanceLoading && !provenanceError && !asset.sourceTaskId && <p className="empty-state">暂无生成历史；该资产没有来源任务。</p>}
            {!provenanceLoading && !provenanceError && asset.sourceTaskId && (
              <>
                <dl className="asset-detail-definition-list">
                  <div><dt>Prompt Version</dt><dd>未记录（历史数据未提供显式提示词版本关联）</dd></div>
                  <div><dt>Model Version</dt><dd>未记录（历史数据未提供显式模型版本关联）</dd></div>
                  <div><dt>Tool Version</dt><dd>{toolVersionLabel(toolUsages)}</dd></div>
                  <div><dt>Source Task</dt><dd>{asset.sourceTaskId}</dd></div>
                </dl>
                <div className="asset-detail-section-heading"><div><strong>Tool Usage</strong><small>工具使用记录来自 ProvenanceLineageService。</small></div><span className="status-pill">{toolUsages.length} 条</span></div>
                {toolUsages.length > 0 ? (
                  <ul className="asset-relation-list">
                    {toolUsages.map((usage) => (
                      <li key={usage.id}>
                        <strong>{usage.toolVersionId ?? "工具版本未记录"}</strong>
                        <span>实例 {usage.toolInstanceId}</span>
                        <small>{formatDateTime(usage.createdAt)}</small>
                      </li>
                    ))}
                  </ul>
                ) : <p className="empty-state">暂无显式工具使用记录。</p>}
                <div className="asset-detail-section-heading"><div><strong>Asset Versions</strong><small>只展示当前资产版本已建立的 Generation → AssetVersion 关系。</small></div><span className="status-pill">{visibleAssetVersionLinks.length} 条</span></div>
                {visibleAssetVersionLinks.length > 0 ? (
                  <ul className="asset-relation-list">
                    {visibleAssetVersionLinks.map((link) => (
                      <li key={link.id}>
                        <strong>{link.assetVersionId}</strong>
                        <span>{link.relationType} · 输出 {link.outputId} · 第 {link.ordinal + 1} 项</span>
                        <small>{formatDateTime(link.createdAt)}</small>
                      </li>
                    ))}
                  </ul>
                ) : <p className="empty-state">暂无显式 AssetVersion 溯源；旧任务可能没有历史关联。</p>}
              </>
            )}
          </section>
        </section>
        {isImage && (
          <section className="asset-video-prompt-panel" aria-label="视频提示词">
            <div className="asset-video-prompt-heading">
              <div>
                <strong>视频提示词</strong>
                <small>资产可直接用于 H3 批量视频；允许保留内部换行。</small>
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
        <AssetUsagePanel projectId={projectId} assetId={asset.id} assetName={displayName} onOpenShot={onOpenShot} onOpenTask={onOpenTask} />
      </section>
    </div>
  );
}

function relationTypeLabel(value: string): string {
  const labels: Record<string, string> = {
    SOURCE_OF: "来源于",
    DERIVED_FROM: "派生自",
    VARIANT_OF: "变体",
    REFERENCE: "参考",
    REPLACEMENT: "替代",
    RELATED: "相关",
  };
  return labels[value] ?? value;
}

function toAssetDetailError(value: unknown): string {
  if (value instanceof Error && value.message) return value.message;
  if (typeof value === "string" && value) return value;
  return "请稍后重试。";
}

function toolVersionLabel(usages: readonly GenerationToolUsageView[]): string {
  const versions = [...new Set(usages.map((usage) => usage.toolVersionId).filter((value): value is string => Boolean(value)))];
  return versions.length ? versions.join("、") : "未记录";
}
