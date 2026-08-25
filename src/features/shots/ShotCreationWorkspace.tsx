import { useEffect, useMemo, useState } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import { formatDateTime, taskStatusLabel } from "../../i18n/statusLabels";
import type { AssetView } from "../../types/asset";
import type { ShotGenerationLink, ShotStage, ShotView } from "../../types/shot";
import { statusLabel } from "./shotDomain";
import { ShotInspector, type ShotInspectorProps, type ShotInspectorTab } from "./ShotInspector";
import { ZoomableImagePreview } from "./ZoomableImagePreview";
import "./ShotCreationWorkspace.css";
import "./ShotInspector.css";

export type ShotCreationWorkspaceTab = "generate" | "references" | "history" | "settings";

export type ShotCandidateStatus = "ready" | "selected" | "reviewed" | "failed" | "generating" | "queued";

export interface ShotWorkspaceCandidate {
  asset: AssetView;
  status?: ShotCandidateStatus;
  statusLabel?: string;
  taskId?: string;
  createdAt?: string;
  error?: string;
  fromLinkedTask?: boolean;
}

export interface ShotCreationWorkspaceProps extends Omit<ShotInspectorProps, "projectId" | "stage" | "activeTab" | "onTabChange"> {
  projectId: string;
  shot?: ShotView;
  stage: ShotStage;
  onStageChange: (stage: ShotStage) => void;
  name?: string;
  onNameChange?: (name: string) => void;
  candidates?: ShotWorkspaceCandidate[];
  selectedAssetId?: string;
  previewAsset?: AssetView;
  onCandidateSelect?: (candidate: ShotWorkspaceCandidate) => void;
  onCandidateConfirm?: (assetId: string, fromLinkedTask?: boolean) => void | Promise<void>;
  onOpenTask?: (taskId: string) => void;
  history?: ShotGenerationLink[];
  onRetry?: (link: ShotGenerationLink) => void | Promise<void>;
  onDeleteShot?: () => void | Promise<void>;
  onCreateShot?: () => void | Promise<void>;
  onCopyPrompt?: (prompt: string) => void;
  workspaceTab?: ShotCreationWorkspaceTab;
  onWorkspaceTabChange?: (tab: ShotCreationWorkspaceTab) => void;
  inspectorTab?: ShotInspectorTab;
  onInspectorTabChange?: (tab: ShotInspectorTab) => void;
  notice?: string;
  error?: string;
}

const workspaceTabs: Array<{ id: ShotCreationWorkspaceTab; label: string }> = [
  { id: "generate", label: "生成" },
  { id: "references", label: "参考" },
  { id: "history", label: "历史" },
  { id: "settings", label: "设置" },
];

const candidateStatusLabels: Record<ShotCandidateStatus, string> = {
  ready: "待审核",
  selected: "已确认",
  reviewed: "已审核",
  failed: "失败",
  generating: "生成中",
  queued: "排队中",
};

export function resolveShotPreviewAsset(candidates: ShotWorkspaceCandidate[], selectedAssetId?: string, previewAsset?: AssetView): AssetView | undefined {
  return previewAsset ?? candidates.find((candidate) => candidate.asset.id === selectedAssetId)?.asset ?? candidates[0]?.asset;
}

export function canConfirmShotCandidate(candidate: ShotWorkspaceCandidate, selectedAssetId?: string): boolean {
  return candidate.asset.id !== selectedAssetId && candidate.status !== "selected" && candidate.status !== "reviewed";
}

export function ShotCreationWorkspace({
  projectId,
  shot,
  stage,
  onStageChange,
  name,
  onNameChange,
  candidates = [],
  selectedAssetId,
  previewAsset,
  onCandidateSelect,
  onCandidateConfirm,
  onOpenTask,
  history,
  onRetry,
  onDeleteShot,
  onCreateShot,
  onCopyPrompt,
  workspaceTab,
  onWorkspaceTabChange,
  inspectorTab,
  onInspectorTabChange,
  notice,
  error,
  ...inspectorProps
}: ShotCreationWorkspaceProps) {
  const [uncontrolledWorkspaceTab, setUncontrolledWorkspaceTab] = useState<ShotCreationWorkspaceTab>("generate");
  const [uncontrolledInspectorTab, setUncontrolledInspectorTab] = useState<ShotInspectorTab>("parameters");
  const selectedWorkspaceTab = workspaceTab ?? uncontrolledWorkspaceTab;
  const selectedInspectorTab = inspectorTab ?? uncontrolledInspectorTab;
  const preview = useMemo(() => resolveShotPreviewAsset(candidates, selectedAssetId, previewAsset), [candidates, previewAsset, selectedAssetId]);
  const stageHistory = history ?? shot?.generationLinks.filter((link) => link.stage === stage) ?? [];
  const promptText = inspectorProps.promptText ?? shot?.promptText ?? "";
  const orderedReferences = inspectorProps.references ?? [];

  function selectWorkspaceTab(tab: ShotCreationWorkspaceTab) {
    if (workspaceTab === undefined) setUncontrolledWorkspaceTab(tab);
    onWorkspaceTabChange?.(tab);
  }

  function selectInspectorTab(tab: ShotInspectorTab) {
    if (inspectorTab === undefined) setUncontrolledInspectorTab(tab);
    onInspectorTabChange?.(tab);
  }

  if (!shot) {
    return (
      <section className="shot-creation-workspace shot-creation-workspace-empty" aria-label="镜头制作">
        <EmptyWorkspaceState title="选择一个镜头开始制作" detail="镜头的提示词、阶段配置、参考关系和候选审核都会在这里集中处理。" actionLabel="新建镜头" onAction={onCreateShot} />
      </section>
    );
  }

  return (
    <section className="shot-creation-workspace" aria-busy={inspectorProps.busy ?? false}>
      <header className="shot-creation-header">
        <div className="shot-creation-title">
          <span className="shot-creation-kicker">镜头 {String(shot.ordinal + 1).padStart(2, "0")}</span>
          <input className="shot-creation-name" value={name ?? shot.name} onChange={(event) => onNameChange?.(event.target.value)} disabled={!onNameChange || inspectorProps.busy} aria-label="镜头名称" />
          <span className="shot-creation-status">{statusLabel(stage === "image" ? shot.imageStatus : shot.videoStatus)}</span>
        </div>
        <div className="shot-creation-stage-switch" role="group" aria-label="媒体阶段">
          <button type="button" aria-pressed={stage === "image"} className={stage === "image" ? "shot-stage-switch-active" : ""} onClick={() => onStageChange("image")} disabled={inspectorProps.busy}>图片</button>
          <button type="button" aria-pressed={stage === "video"} className={stage === "video" ? "shot-stage-switch-active" : ""} onClick={() => onStageChange("video")} disabled={inspectorProps.busy}>视频</button>
        </div>
      </header>

      <nav className="shot-creation-tabs" role="tablist" aria-label="镜头工作区">
        {workspaceTabs.map((tab) => <button key={tab.id} type="button" role="tab" aria-selected={selectedWorkspaceTab === tab.id} className={selectedWorkspaceTab === tab.id ? "shot-creation-tab shot-creation-tab-active" : "shot-creation-tab"} onClick={() => selectWorkspaceTab(tab.id)}>{tab.label}</button>)}
      </nav>

      <div className="shot-creation-layout">
        <main className="shot-creation-main">
          {selectedWorkspaceTab === "generate" && <GenerateWorkspace
            projectId={projectId}
            stage={stage}
            candidates={candidates}
            selectedAssetId={selectedAssetId}
            previewAsset={preview}
            onCandidateSelect={onCandidateSelect}
            onCandidateConfirm={onCandidateConfirm}
            previewResetKey={`${shot.id}:${stage}`}
            onGenerate={inspectorProps.onGenerate}
            busy={inspectorProps.busy}
            promptText={promptText}
            onCopyPrompt={onCopyPrompt}
            onEditPrompt={() => selectInspectorTab("prompt")}
          />}
          {selectedWorkspaceTab === "references" && <ReferenceWorkspace projectId={projectId} stage={stage} references={orderedReferences} keyframeAsset={inspectorProps.keyframeAsset} />}
          {selectedWorkspaceTab === "history" && <HistoryWorkspace history={stageHistory} onOpenTask={onOpenTask} onRetry={onRetry} busy={inspectorProps.busy ?? false} />}
          {selectedWorkspaceTab === "settings" && <SettingsWorkspace shot={shot} onDeleteShot={onDeleteShot} busy={inspectorProps.busy ?? false} />}
        </main>
        <ShotInspector {...inspectorProps} projectId={projectId} stage={stage} promptText={promptText} activeTab={selectedInspectorTab} onTabChange={selectInspectorTab} />
      </div>
      {(notice || error) && <div className={error ? "shot-creation-feedback shot-creation-feedback-error" : "shot-creation-feedback"} role={error ? "alert" : "status"}>{error ?? notice}</div>}
    </section>
  );
}

function GenerateWorkspace({ projectId, stage, candidates, selectedAssetId, previewAsset, onCandidateSelect, onCandidateConfirm, previewResetKey, onGenerate, busy, promptText, onCopyPrompt, onEditPrompt }: {
  projectId: string;
  stage: ShotStage;
  candidates: ShotWorkspaceCandidate[];
  selectedAssetId?: string;
  previewAsset?: AssetView;
  onCandidateSelect?: (candidate: ShotWorkspaceCandidate) => void;
  onCandidateConfirm?: (assetId: string, fromLinkedTask?: boolean) => void | Promise<void>;
  previewResetKey: string;
  onGenerate: () => void | Promise<void>;
  busy?: boolean;
  promptText: string;
  onCopyPrompt?: (prompt: string) => void;
  onEditPrompt: () => void;
}) {
  const previewLabel = previewAsset ? (stage === "image" ? "当前图片预览" : "当前视频预览") : "尚未生成候选";
  const previewCandidate = previewAsset ? candidates.find((candidate) => candidate.asset.id === previewAsset.id) : undefined;
  const canConfirmPreview = Boolean(
    onCandidateConfirm &&
    previewCandidate &&
    previewCandidate.asset.id !== selectedAssetId &&
    canConfirmShotCandidate(previewCandidate, selectedAssetId),
  );
  return (
    <div className="shot-creation-view shot-creation-generate-view">
      <div className="shot-creation-view-heading"><div><span className="shot-creation-kicker">生成</span><h2>{stage === "image" ? "关键帧图片" : "镜头视频"}</h2></div><span className="shot-creation-count">{candidates.length} 个候选</span></div>
      <section className="shot-preview-candidate-shell" aria-label="主预览与候选">
        <div className="shot-main-preview">
          <div className="shot-main-preview-heading"><span>{previewLabel}</span>{previewAsset && <small>{previewAsset.name}</small>}</div>
          {previewAsset ? <ShotMediaPreview projectId={projectId} asset={previewAsset} variant="main" resetKey={`${previewResetKey}:${previewAsset.id}`} /> : <EmptyWorkspaceState title="尚未生成候选" detail="生成完成后，候选会出现在右侧；确认动作仍由你显式触发。" actionLabel={busy ? "生成中…" : "生成第一个候选"} onAction={onGenerate} disabled={busy} />}
        </div>
        <aside className="shot-candidate-rail" aria-label="候选列表">
          <div className="shot-candidate-rail-heading"><strong>候选</strong><span>{candidates.length}</span></div>
          <div className="shot-candidate-list">
            {candidates.map((candidate) => <CandidateRailItem key={`${candidate.asset.id}:${candidate.taskId ?? "candidate"}`} projectId={projectId} candidate={candidate} selected={candidate.status === "selected" || candidate.asset.id === selectedAssetId} onSelect={onCandidateSelect} disabled={busy ?? false} />)}
            {candidates.length === 0 && <p className="shot-creation-muted">暂无候选</p>}
          </div>
          <button type="button" className="shot-candidate-confirm" onClick={() => previewCandidate && void onCandidateConfirm?.(previewCandidate.asset.id, previewCandidate.fromLinkedTask)} disabled={busy || !canConfirmPreview}>确认当前候选</button>
        </aside>
      </section>
      <section className="shot-prompt-preview" aria-label="提示词预览">
        <div className="shot-prompt-preview-heading"><div><span className="shot-creation-kicker">提示词预览</span><strong>当前实际提示词</strong></div><div className="shot-prompt-preview-actions"><button type="button" className="quiet-button" onClick={() => onCopyPrompt?.(promptText)} disabled={!onCopyPrompt || !promptText.trim()}>复制</button><button type="button" className="quiet-button" onClick={onEditPrompt}>编辑</button></div></div>
        <p>{promptText || "尚未填写提示词。"}</p>
      </section>
    </div>
  );
}

function CandidateRailItem({ projectId, candidate, selected, onSelect, disabled }: { projectId: string; candidate: ShotWorkspaceCandidate; selected: boolean; onSelect?: (candidate: ShotWorkspaceCandidate) => void; disabled: boolean }) {
  const status = candidate.statusLabel ?? (candidate.status ? candidateStatusLabels[candidate.status] : "待审核");
  return (
    <article className={`shot-candidate-rail-item${selected ? " shot-candidate-rail-item-selected" : ""}`}>
      <button type="button" className="shot-candidate-select" onClick={() => onSelect?.(candidate)} disabled={disabled || !onSelect} aria-pressed={selected}>
        <span className="shot-candidate-thumb"><ShotMediaPreview projectId={projectId} asset={candidate.asset} variant="thumb" /><span className={`shot-candidate-status shot-candidate-status-${candidate.status ?? "ready"}`}>{status}</span></span>
        <span className="shot-candidate-copy"><strong>{candidate.asset.name}</strong></span>
      </button>
      {candidate.error && <small className="shot-candidate-error">{candidate.error}</small>}
    </article>
  );
}

function ReferenceWorkspace({ projectId, stage, references, keyframeAsset }: { projectId: string; stage: ShotStage; references: ShotInspectorProps["references"]; keyframeAsset?: AssetView }) {
  return (
    <div className="shot-creation-view">
      <div className="shot-creation-view-heading"><div><span className="shot-creation-kicker">参考素材</span><h2>参考素材总览</h2></div><span className="shot-creation-count">{references?.length ?? 0} 张</span></div>
      {stage === "video" && keyframeAsset && <div className="shot-reference-keyframe"><ShotMediaPreview projectId={projectId} asset={keyframeAsset} variant="thumb" /><span><small>关键帧</small><strong>{keyframeAsset.name}</strong></span></div>}
      <div className="shot-reference-grid">
        {references?.map((reference, index) => <article key={`${reference.assetId}:${index}`} className="shot-reference-card"><span className="shot-reference-index">@图片{index + 1}</span>{reference.asset ? <ShotMediaPreview projectId={projectId} asset={reference.asset} variant="card" /> : <div className="shot-reference-placeholder">暂无预览</div>}<strong>{reference.label ?? reference.asset?.name ?? reference.assetId}</strong><small>{reference.assetId}</small></article>)}
        {!references?.length && <EmptyWorkspaceState title="暂无参考素材" detail="在右侧参数面板的“参考”页添加当前镜头的有序参考图。" />}
      </div>
    </div>
  );
}

function HistoryWorkspace({ history, onOpenTask, onRetry, busy }: { history: ShotGenerationLink[]; onOpenTask?: (taskId: string) => void; onRetry?: (link: ShotGenerationLink) => void | Promise<void>; busy: boolean }) {
  return (
    <div className="shot-creation-view">
      <div className="shot-creation-view-heading"><div><span className="shot-creation-kicker">历史</span><h2>生成历史</h2></div><span className="shot-creation-count">{history.length} 条</span></div>
      <div className="shot-history-table">
        {history.map((link) => {
          const statusCode = link.task?.status;
          const status = statusCode ? taskStatusLabel(statusCode) : (link.productionBatchItemId ? taskStatusLabel("QUEUED") : "关联中");
          const taskId = link.task?.id ?? link.taskId;
          return <article key={link.id} className="shot-history-row"><span className="shot-history-date">{formatDateTime(link.createdAt)}</span><span className="shot-history-stage">{link.stage === "image" ? "图片" : "视频"}</span><strong>{status}</strong><span className="shot-history-output">{link.task?.outputAssetIds.length ?? 0} 个候选</span><div className="shot-history-actions">{taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(taskId)}>查看任务</button>}{statusCode === "FAILED" && onRetry && <button type="button" className="quiet-button" onClick={() => void onRetry(link)} disabled={busy}>重试</button>}</div></article>;
        })}
        {history.length === 0 && <EmptyWorkspaceState title="尚无生成历史" detail="当前阶段的任务、候选和失败记录会按时间出现在这里。" />}
      </div>
    </div>
  );
}

function SettingsWorkspace({ shot, onDeleteShot, busy }: { shot: ShotView; onDeleteShot?: () => void | Promise<void>; busy: boolean }) {
  return (
    <div className="shot-creation-view shot-settings-view">
      <div className="shot-creation-view-heading"><div><span className="shot-creation-kicker">设置</span><h2>镜头信息</h2></div></div>
      <dl className="shot-settings-metadata"><div><dt>镜头 ID</dt><dd>{shot.id}</dd></div><div><dt>状态</dt><dd>{statusLabel(shot.status)}</dd></div><div><dt>创建时间</dt><dd>{formatDateTime(shot.createdAt)}</dd></div><div><dt>更新时间</dt><dd>{formatDateTime(shot.updatedAt)}</dd></div></dl>
      <section className="shot-danger-zone"><div><span className="shot-creation-kicker">危险操作</span><h3>删除镜头</h3><p>只删除镜头编排元数据，不会自动删除已生成的素材文件。</p></div><button type="button" className="danger-button" onClick={() => void onDeleteShot?.()} disabled={busy || !onDeleteShot}>删除镜头</button></section>
    </div>
  );
}

function EmptyWorkspaceState({ title, detail, actionLabel, onAction, disabled = false }: { title: string; detail: string; actionLabel?: string; onAction?: () => void | Promise<void>; disabled?: boolean }) {
  return <div className="shot-workspace-empty-state"><span className="shot-empty-glyph" aria-hidden="true">＋</span><strong>{title}</strong><p>{detail}</p>{actionLabel && onAction && <button type="button" onClick={() => void onAction()} disabled={disabled}>{actionLabel}</button>}</div>;
}

function ShotMediaPreview({ projectId, asset, variant, resetKey }: { projectId: string; asset: AssetView; variant: "main" | "card" | "thumb"; resetKey?: string }) {
  const isVideo = asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
  const media = useAssetMedia(projectId, asset, variant, isVideo);
  if (isVideo && media.src) return <video className={`shot-media-preview shot-media-preview-${variant}`} src={media.src} poster={media.poster} controls={variant === "main"} preload="metadata" muted={variant !== "main"} playsInline aria-label={asset.name} />;
  if (media.src) {
    if (variant === "main") return <ZoomableImagePreview imageUrl={media.src} alt={asset.name} label={`${asset.name} 主预览`} resetKey={resetKey} className={`shot-media-preview shot-media-preview-${variant}`} />;
    return <img className={`shot-media-preview shot-media-preview-${variant}`} src={media.src} alt={asset.name} loading="lazy" />;
  }
  return <div className={`shot-media-preview shot-media-preview-${variant} shot-media-placeholder`} aria-label={`${asset.name} 预览加载中`}>预览</div>;
}

function useAssetMedia(projectId: string, asset: AssetView, variant: "main" | "card" | "thumb", isVideo: boolean): { src?: string; poster?: string } {
  const [media, setMedia] = useState<{ src?: string; poster?: string }>(() => isVideo ? { src: getAssetMediaUrl(projectId, asset.id, "video") } : {});
  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    let posterUrl: string | undefined;
    setMedia(isVideo ? { src: getAssetMediaUrl(projectId, asset.id, "video") } : {});
    const readPreview = isVideo
      ? asset.thumbnailAvailable ? readAssetThumbnail(projectId, asset.id) : undefined
      : variant === "main"
        ? readAssetImage(projectId, asset.id)
        : asset.thumbnailAvailable ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id)) : readAssetImage(projectId, asset.id);
    if (!readPreview) return () => { active = false; };
    void readPreview.then((bytes) => {
      if (!active) return;
      const nextUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      if (isVideo) {
        posterUrl = nextUrl;
        setMedia((current) => ({ ...current, poster: nextUrl }));
      } else {
        objectUrl = nextUrl;
        setMedia({ src: nextUrl });
      }
    }).catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      if (posterUrl) URL.revokeObjectURL(posterUrl);
    };
  }, [asset.id, asset.mimeType, asset.thumbnailAvailable, isVideo, projectId, variant]);
  return media;
}
