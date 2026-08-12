import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  assetLibraryPage,
  commitH3LocalImport,
  createGeneration,
  createProductionQueue,
  readAssetImage,
  readAssetThumbnail,
  listAssetVideoPrompts,
  pickH3LocalImportDirectory,
  rescanH3LocalImport,
  setAssetVideoPrompt,
  startProductionQueue,
  updateH3ProjectSegmentDraft,
} from "../../services/tauriClient";
import type { AssetMediaTypeFilter, AssetView, PageCursor } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import type { GenerationValues } from "../../types/generation";
import type {
  H3LocalImportInspection,
  H3ProjectSegment,
  H3ProjectGenerationMode,
  H3ProjectFolderInspection,
} from "../../types/h3LocalImport";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { toUserMessage } from "../../i18n/errorMessages";
import {
  H3_QUALITY_PROFILE,
  h3RecipeForMode,
  MINIMAX_H3_FL2VA_WORKFLOW_ID,
  MINIMAX_H3_WORKFLOW_ID,
  type H3QualityProfile,
} from "../runtime/productRuntimeScope";
import { ResolutionControl } from "../runtime/ResolutionControl";
import {
  isMinimaxH3OutputResolution,
  MINIMAX_H3_RESOLUTION_PRESETS,
  resolutionPresetsForRecipe,
} from "../runtime/resolutionPresets";
import { validateResolution } from "../runtime/resolution";
import {
  buildH3BatchDraft,
  H3_MODE_OPTIONS,
  h3ModeSupported,
  type H3GenerationMode,
  h3AssetQualification,
  h3RecipeContract,
  isAudioAssetForH3,
  isImageAssetForVideo,
  isVideoAssetForH3,
} from "./assetVideoBatch";
import { ProductionQueuePanel } from "../studio/ProductionQueuePanel";
import type { BatchDraftItem } from "../studio/batchDraft";
import { formatPromptBytes, localImportCanCommit, localImportStatusLabel } from "./h3LocalImport";
import { AssetCard } from "./AssetCard";
import { DynamicFormRenderer, validateRecipeValues } from "../studio/DynamicFormRenderer";
import { WorkflowSelector } from "../runtime/WorkflowSelector";
import { defaultGenerationValues } from "../../stores/studioStore";
import {
  clearSelectedRecipeRef,
  filterVideoRecipes,
  readSelectedRecipeRef,
  readProjectWorkflowOverrides,
  resolveProjectFolderRecipes,
  resolveVideoRecipe,
  recipeRef,
  videoRecipeCapability,
  writeSelectedRecipeRef,
  writeProjectWorkflowOverrides,
  type H3CompatibleMode,
  type SelectedRecipeRef,
} from "../runtime/workflowCapabilities";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  initialAssets: AssetView[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  productionAdmission: ProductionAdmissionStatus;
  focusProductionBatchId?: string;
  onAdmissionChanged: () => Promise<void>;
  onProductionBatchFocused: () => void;
  onOpenTask: (taskId: string) => void;
  onBackToAssets: () => void;
  onOpenWorkflows?: () => void;
}

interface H3AssetLibraryPickerProps {
  projectId: string;
  assets: AssetView[];
  generationMode: H3GenerationMode;
  selectedIds: Set<string>;
  firstFrameAssetId?: string;
  lastFrameAssetId?: string;
  keyword: string;
  mediaType: AssetMediaTypeFilter;
  loading: boolean;
  error?: string;
  hasMore: boolean;
  busy: boolean;
  onKeywordChange: (value: string) => void;
  onMediaTypeChange: (value: AssetMediaTypeFilter) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
  onToggleAsset: (asset: AssetView) => void;
  onSetFirstFrame: (asset: AssetView) => void;
  onSetLastFrame: (asset: AssetView) => void;
}

export function h3InitialGenerationMode(initialAssets: AssetView[]): H3GenerationMode {
  return initialAssets.length === 1 && isImageAssetForVideo(initialAssets[0])
    ? "FL2VA_IMAGE_TO_VIDEO"
    : "FL2VA_TEXT_TO_VIDEO";
}

function videoWorkflowCandidatesForMode(catalog: RecipeViewModel[], mode: H3GenerationMode): RecipeViewModel[] {
  const compatible = catalog.filter((candidate) => videoRecipeCapability(candidate).supportedModes.includes(mode));
  return compatible.length
    ? compatible
    : catalog.filter((candidate) => videoRecipeCapability(candidate).supportedModes.includes("CUSTOM_VIDEO"));
}

export function h3PickerAssets(assets: AssetView[], mode: H3GenerationMode): AssetView[] {
  switch (mode) {
    case "FL2VA_IMAGE_TO_VIDEO":
    case "FL2VA_FIRST_LAST":
    case "REF2VA_IMAGE":
      return assets.filter(isImageAssetForVideo);
    case "REF2VA_AUDIO":
      return assets.filter(isAudioAssetForH3);
    case "REF2VA_IMAGE_AUDIO":
      return assets.filter((asset) => isImageAssetForVideo(asset) || isAudioAssetForH3(asset));
    case "REF2VA_VIDEO_IMAGE":
      return assets.filter((asset) => isImageAssetForVideo(asset) || isVideoAssetForH3(asset));
    case "FL2VA_TEXT_TO_VIDEO":
      return [];
  }
}

function H3SelectedFrame({ projectId, label, asset }: { projectId: string; label: string; asset?: AssetView }) {
  const [previewUrl, setPreviewUrl] = useState<string>();

  useEffect(() => {
    if (!asset) {
      setPreviewUrl(undefined);
      return () => undefined;
    }
    let active = true;
    let url: string | undefined;
    void readAssetThumbnail(projectId, asset.id)
      .catch(() => readAssetImage(projectId, asset.id))
      .then((bytes) => {
        if (!active) return;
        url = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType || "image/png" }));
        setPreviewUrl(url);
      })
      .catch(() => {
        if (active) setPreviewUrl(undefined);
      });
    return () => {
      active = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [asset, projectId]);

  return (
    <div className="h3-frame-selection-summary">
      <span className="h3-frame-selection-preview">
        {previewUrl ? <img src={previewUrl} alt={asset?.name ?? label} /> : <span>{asset ? "加载预览…" : "未选择"}</span>}
      </span>
      <span className="h3-frame-selection-copy">
        <small>{label}</small>
        <strong>{asset?.name ?? "未选择图片"}</strong>
      </span>
    </div>
  );
}

function H3AssetLibraryPicker({
  projectId,
  assets,
  generationMode,
  selectedIds,
  firstFrameAssetId,
  lastFrameAssetId,
  keyword,
  mediaType,
  loading,
  error,
  hasMore,
  busy,
  onKeywordChange,
  onMediaTypeChange,
  onRefresh,
  onLoadMore,
  onToggleAsset,
  onSetFirstFrame,
  onSetLastFrame,
}: H3AssetLibraryPickerProps) {
  const pickerAssets = h3PickerAssets(assets, generationMode);
  const firstFrame = assets.find((asset) => asset.id === firstFrameAssetId);
  const lastFrame = assets.find((asset) => asset.id === lastFrameAssetId);
  const referenceMode = generationMode.startsWith("REF2VA");

  return (
    <section className="h3-asset-library-picker" aria-label="H3 资产库媒体选择">
      <div className="h3-asset-library-picker-heading">
        <div>
          <span className="section-label">Asset Library</span>
          <h3>选择输入素材</h3>
          <p className="section-description">
            {generationMode === "FL2VA_TEXT_TO_VIDEO"
              ? "当前文生视频模式不需要媒体输入。"
              : referenceMode
                ? "从当前项目资产库选择参考媒体；选择顺序会冻结到本次批次。"
                : "从当前项目资产库选择首帧或首尾帧图片。"}
          </p>
        </div>
        <button type="button" className="quiet-button" onClick={onRefresh} disabled={busy || loading}>
          {loading ? "正在加载…" : "刷新资产库"}
        </button>
      </div>

      {generationMode !== "FL2VA_TEXT_TO_VIDEO" && (
        <>
          <div className="h3-asset-library-picker-controls">
            <label className="h3-asset-library-search">
              <span>搜索素材</span>
              <input value={keyword} onChange={(event) => onKeywordChange(event.target.value)} placeholder="搜索名称或原始文件名" disabled={busy} />
            </label>
            <label>
              <span>媒体类型</span>
              <select value={mediaType} onChange={(event) => onMediaTypeChange(event.target.value as AssetMediaTypeFilter)} disabled={busy}>
                <option value="ALL">全部</option>
                <option value="IMAGE">图片</option>
                <option value="VIDEO">视频</option>
                <option value="AUDIO">音频</option>
              </select>
            </label>
          </div>
          {generationMode === "FL2VA_IMAGE_TO_VIDEO" && (
            <div className="h3-frame-selection-strip">
              <H3SelectedFrame projectId={projectId} label="首帧图片" asset={firstFrame} />
            </div>
          )}
          {generationMode === "FL2VA_FIRST_LAST" && (
            <div className="h3-frame-selection-strip">
              <H3SelectedFrame projectId={projectId} label="首帧图片" asset={firstFrame} />
              <H3SelectedFrame projectId={projectId} label="末帧图片" asset={lastFrame} />
            </div>
          )}
          {error && <p className="error-message" role="alert">资产库加载失败：{error}</p>}
          <div className="h3-asset-library-picker-grid" aria-label="可选资产">
            {pickerAssets.map((asset) => {
              const selected = selectedIds.has(asset.id);
              const isImage = isImageAssetForVideo(asset);
              return (
                <div className={`h3-asset-picker-item${selected ? " h3-asset-picker-item-selected" : ""}`} key={asset.id}>
                  <AssetCard
                    projectId={projectId}
                    asset={asset}
                    onSelect={() => onToggleAsset(asset)}
                    selectionMode={referenceMode}
                    selected={selected}
                    onToggleSelection={() => onToggleAsset(asset)}
                  />
                  {(generationMode === "FL2VA_IMAGE_TO_VIDEO" || generationMode === "FL2VA_FIRST_LAST") && isImage && (
                    <div className="h3-asset-picker-role-actions">
                      <button type="button" className={firstFrameAssetId === asset.id ? "quiet-button asset-role-active" : "quiet-button"} onClick={() => onSetFirstFrame(asset)} disabled={busy || lastFrameAssetId === asset.id}>设为首帧</button>
                      {generationMode === "FL2VA_FIRST_LAST" && <button type="button" className={lastFrameAssetId === asset.id ? "quiet-button asset-role-active" : "quiet-button"} onClick={() => onSetLastFrame(asset)} disabled={busy || firstFrameAssetId === asset.id}>设为末帧</button>}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          {!loading && !pickerAssets.length && !error && <p className="empty-state">当前筛选下没有可用的媒体资产。</p>}
          {hasMore && <button type="button" className="load-more-button quiet-button" onClick={onLoadMore} disabled={busy || loading}>加载更多</button>}
        </>
      )}
    </section>
  );
}

interface ProjectSegmentForm {
  mode: H3ProjectGenerationMode;
  prompt: string;
  durationSeconds: number;
  width: number;
  height: number;
  referenceImageIds: string[];
  referenceAudioIds: string[];
  referenceVideoIds: string[];
  firstFrameId?: string;
  lastFrameId?: string;
}

interface ProjectFolderImportControlsProps {
  busy: boolean;
  hasInspection: boolean;
  onRescan: () => void;
}

export function ProjectFolderImportControls({
  busy,
  hasInspection,
  onRescan,
}: ProjectFolderImportControlsProps) {
  return (
    <div className="h3-local-import-controls" aria-label="项目文件夹导入规则">
      <div className="h3-project-folder-policy">
        <strong>项目文件夹 · Segment 自动识别</strong>
        <span>每个一级子文件夹对应一个视频 Segment；系统按 Prompt 和媒体自动选择生成模式。</span>
      </div>
      {hasInspection && (
        <button type="button" className="quiet-button" onClick={onRescan} disabled={busy}>
          重新扫描
        </button>
      )}
    </div>
  );
}

interface ProjectFolderWorkflowStrategyProps {
  strategy: "AUTO" | "MANUAL";
  modes: H3CompatibleMode[];
  catalog: RecipeViewModel[];
  recommendations: Partial<Record<H3CompatibleMode, SelectedRecipeRef | undefined>>;
  overrides: Partial<Record<H3CompatibleMode, SelectedRecipeRef | undefined>>;
  resolved: Array<{ mode: H3CompatibleMode; recipe?: RecipeViewModel; source: "manual" | "recommended" | "compatible"; staleManualSelection: boolean }>;
  busy: boolean;
  onStrategyChange: (strategy: "AUTO" | "MANUAL") => void;
  onOverrideChange: (mode: H3CompatibleMode, ref: SelectedRecipeRef | undefined) => void;
}

function ProjectFolderWorkflowStrategy({
  strategy,
  modes,
  catalog,
  recommendations,
  overrides,
  resolved,
  busy,
  onStrategyChange,
  onOverrideChange,
}: ProjectFolderWorkflowStrategyProps) {
  const modeLabel = (mode: H3CompatibleMode) => H3_MODE_OPTIONS.find((option) => option.id === mode)?.label ?? mode;
  const candidatesFor = (mode: H3CompatibleMode) => catalog.filter((recipe) => (
    videoRecipeCapability(recipe).projectFolderModes.includes(mode)
  ));

  return (
    <section className="project-folder-workflow-strategy" aria-label="项目文件夹工作流策略">
      <div className="project-folder-workflow-strategy-heading">
        <div>
          <span className="section-label">工作流策略</span>
          <strong>项目文件夹按模式选择</strong>
          <p>自动推荐保持现有 H3 映射；手动模式只会冻结当前项目中实际出现的 Segment 模式。</p>
        </div>
        <label>
          <span>策略</span>
          <select value={strategy} onChange={(event) => onStrategyChange(event.target.value as "AUTO" | "MANUAL")} disabled={busy}>
            <option value="AUTO">自动推荐</option>
            <option value="MANUAL">手动按模式指定</option>
          </select>
        </label>
      </div>
      <div className="project-folder-workflow-modes">
        {modes.map((mode) => {
          const candidates = candidatesFor(mode);
          const current = resolved.find((item) => item.mode === mode)?.recipe;
          const staleManualSelection = resolved.find((item) => item.mode === mode)?.staleManualSelection ?? false;
          const override = overrides[mode];
          const recommended = recommendations[mode];
          const selectedValue = override ? `${override.workflowVersionId}:${override.recipeId}` : "__recommended__";
          return (
            <div className="project-folder-workflow-mode" key={mode}>
              <div>
                <strong>{modeLabel(mode)}</strong>
                <small>{staleManualSelection ? "手动选择已失效，请重新选择兼容 Recipe" : current ? `${current.name} · ${current.workflowVersionId} · ${current.recipeId}` : "没有兼容工作流"}</small>
              </div>
              {strategy === "MANUAL" ? (
                <select
                  aria-label={`${modeLabel(mode)}工作流`}
                  value={selectedValue}
                  onChange={(event) => {
                    if (event.target.value === "__recommended__") {
                      onOverrideChange(mode, undefined);
                      return;
                    }
                    const [workflowVersionId, recipeId] = event.target.value.split(":");
                    onOverrideChange(mode, { workflowVersionId, recipeId });
                  }}
                  disabled={busy || !candidates.length}
                >
                  <option value="__recommended__">自动推荐 · {recommended ? "当前推荐" : "无推荐"}</option>
                  {candidates.map((candidate) => (
                    <option key={`${candidate.workflowVersionId}:${candidate.recipeId}`} value={`${candidate.workflowVersionId}:${candidate.recipeId}`}>
                      {candidate.name} · {candidate.workflowVersionId} · {candidate.recipeId}
                    </option>
                  ))}
                </select>
              ) : (
                <span className={current ? "project-folder-workflow-source" : "project-folder-workflow-source project-folder-workflow-source-error"}>
                  {current ? "自动推荐" : "缺少兼容 Recipe"}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

export function projectParameterSourceLabel(source: string): string {
  switch (source) {
    case "USER_OVERRIDE": return "用户修改";
    case "FRONT_MATTER": return "Front Matter";
    case "PROMPT_SPEC": return "提示词";
    case "PROMPT_SPEC_ROUNDED": return "提示词取整";
    case "REFERENCE_VIDEO": return "参考视频";
    case "SOURCE_ASPECT": return "素材比例";
    case "RECIPE_DEFAULT": return "默认值";
    default: return source;
  }
}

function projectSegmentForm(segment: H3ProjectSegment): ProjectSegmentForm {
  return {
    mode: segment.generationMode,
    prompt: segment.prompt ?? "",
    durationSeconds: segment.durationSeconds,
    width: segment.width,
    height: segment.height,
    referenceImageIds: segment.referenceImages.map((media) => media.id),
    referenceAudioIds: segment.referenceAudios.map((media) => media.id),
    referenceVideoIds: segment.referenceVideos.map((media) => media.id),
    firstFrameId: segment.firstFrame?.id,
    lastFrameId: segment.lastFrame?.id,
  };
}

interface ProjectFolderSegmentEditorProps {
  project: H3ProjectFolderInspection;
  forms: Record<string, ProjectSegmentForm>;
  busy: boolean;
  expandedOrdinal?: number;
  onToggle: (ordinal: number) => void;
  onChange: (segmentId: string, patch: Partial<ProjectSegmentForm>) => void;
  onSave: (segment: H3ProjectSegment) => void;
  onReset: (segment: H3ProjectSegment) => void;
}

function ProjectFolderSegmentEditor({
  project,
  forms,
  busy,
  expandedOrdinal,
  onToggle,
  onChange,
  onSave,
  onReset,
}: ProjectFolderSegmentEditorProps) {
  function orderedMedia(
    segment: H3ProjectSegment,
    form: ProjectSegmentForm,
    field: "referenceImageIds" | "referenceAudioIds" | "referenceVideoIds",
    kind: "image" | "audio" | "video",
    label: string,
  ) {
    const media = segment.media.filter((item) => item.kind === kind);
    const ids = form[field];
    const move = (index: number, delta: number) => {
      const next = [...ids];
      const target = index + delta;
      if (target < 0 || target >= next.length) return;
      [next[index], next[target]] = [next[target], next[index]];
      onChange(segment.segmentId, { [field]: next });
    };
    const toggle = (id: string) => {
      onChange(segment.segmentId, {
        [field]: ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id],
      });
    };
    return (
      <div className="h3-project-media-editor">
        <strong>{label}</strong>
        <div className="h3-project-media-selected">
          {ids.map((id, index) => {
            const item = media.find((candidate) => candidate.id === id);
            if (!item) return null;
            return (
              <div className="h3-project-media-chip" key={id}>
                <span>{item.displayName}</span>
                <button type="button" className="quiet-button" onClick={() => move(index, -1)} disabled={busy || index === 0} aria-label={`上移${item.displayName}`}>↑</button>
                <button type="button" className="quiet-button" onClick={() => move(index, 1)} disabled={busy || index === ids.length - 1} aria-label={`下移${item.displayName}`}>↓</button>
                <button type="button" className="quiet-button" onClick={() => toggle(id)} disabled={busy} aria-label={`移除${item.displayName}`}>×</button>
              </div>
            );
          })}
          {!ids.length && <span className="h3-project-media-empty">未选择</span>}
        </div>
        <div className="h3-project-media-available">
          {media.filter((item) => !ids.includes(item.id)).map((item) => (
            <button type="button" className="quiet-button" key={item.id} onClick={() => toggle(item.id)} disabled={busy}>+ {item.displayName}</button>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="h3-project-segment-list" aria-label="项目文件夹 Segment 列表">
      <div className="h3-project-segment-heading">
        <span>按自然排序的 Segment</span>
        <span>{project.readyCount} / {project.segmentCount} 段可生成</span>
      </div>
      {project.segments.map((segment) => {
        const form = forms[segment.segmentId] ?? projectSegmentForm(segment);
        const expanded = expandedOrdinal === segment.ordinal;
        const images = segment.media.filter((item) => item.kind === "image");
        return (
          <article className={`h3-project-segment-card${segment.status === "READY" ? " h3-project-segment-ready" : " h3-project-segment-blocked"}`} key={segment.segmentId}>
            <div className="h3-project-segment-summary">
              <span className="h3-local-import-ordinal">#{segment.ordinal}</span>
              <div className="h3-project-segment-main">
                <strong>{segment.folderName}</strong>
                <span>{H3_MODE_OPTIONS.find((option) => option.id === segment.generationMode)?.label ?? segment.generationMode} · {segment.durationSeconds} 秒 · {segment.width} × {segment.height}</span>
                <small>时长来源：{projectParameterSourceLabel(segment.durationSource)} · 分辨率来源：{projectParameterSourceLabel(segment.resolutionSource)}</small>
                <small>{segment.prompt?.replace(/\s+/g, " ").slice(0, 180) || "缺少 Prompt"}</small>
              </div>
              <div className="h3-project-segment-counts">
                <span>图 {segment.media.filter((item) => item.kind === "image").length}</span>
                <span>音 {segment.media.filter((item) => item.kind === "audio").length}</span>
                <span>视 {segment.media.filter((item) => item.kind === "video").length}</span>
              </div>
              <span className={segment.status === "READY" ? "h3-local-import-status h3-local-import-status-ready" : "h3-local-import-status"}>{segment.status === "READY" ? "可生成" : "需修复"}</span>
              <button type="button" className="quiet-button" onClick={() => onToggle(segment.ordinal)} disabled={busy}>{expanded ? "收起编辑" : "展开编辑"}</button>
            </div>
            {segment.errors.length > 0 && <div className="h3-project-segment-errors" role="alert">{segment.errors.map((error) => <span key={error}>{error}</span>)}</div>}
            {expanded && (
              <div className="h3-project-segment-editor">
                <div className="h3-project-segment-fields">
                  <label><span>生成模式</span><select value={form.mode} onChange={(event) => onChange(segment.segmentId, { mode: event.target.value as H3ProjectGenerationMode })} disabled={busy}>{H3_MODE_OPTIONS.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}</select></label>
                  <label><span>时长（秒）</span><input type="number" min={1} max={15} step={1} value={form.durationSeconds} onChange={(event) => onChange(segment.segmentId, { durationSeconds: Number(event.target.value) })} disabled={busy} /></label>
                  <label>
                    <span>输出分辨率</span>
                    <select
                      value={`${form.width}x${form.height}`}
                      onChange={(event) => {
                        const [width, height] = event.target.value.split("x").map(Number);
                        onChange(segment.segmentId, { width, height });
                      }}
                      disabled={busy}
                    >
                      {!isMinimaxH3OutputResolution(form.width, form.height) && (
                        <option value={`${form.width}x${form.height}`} disabled>
                          当前值不支持：{form.width} × {form.height}
                        </option>
                      )}
                      {MINIMAX_H3_RESOLUTION_PRESETS.map((preset) => (
                        <option key={preset.id} value={`${preset.width}x${preset.height}`}>
                          {preset.label} · {preset.width} × {preset.height}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label className="h3-project-prompt-editor"><span>Prompt（只保存到本次 Session Draft）</span><textarea rows={4} maxLength={64 * 1024} value={form.prompt} onChange={(event) => onChange(segment.segmentId, { prompt: event.target.value })} disabled={busy} /></label>
                {(form.mode === "FL2VA_IMAGE_TO_VIDEO" || form.mode === "FL2VA_FIRST_LAST") && (
                  <div className="h3-project-frame-editor">
                    <label><span>首帧</span><select value={form.firstFrameId ?? ""} onChange={(event) => onChange(segment.segmentId, { firstFrameId: event.target.value || undefined })} disabled={busy}><option value="">未选择</option>{images.map((item) => <option key={item.id} value={item.id}>{item.displayName}</option>)}</select></label>
                    {form.mode === "FL2VA_FIRST_LAST" && <label><span>末帧</span><select value={form.lastFrameId ?? ""} onChange={(event) => onChange(segment.segmentId, { lastFrameId: event.target.value || undefined })} disabled={busy}><option value="">未选择</option>{images.map((item) => <option key={item.id} value={item.id}>{item.displayName}</option>)}</select></label>}
                  </div>
                )}
                {orderedMedia(segment, form, "referenceImageIds", "image", "参考图片顺序")}
                {orderedMedia(segment, form, "referenceAudioIds", "audio", "参考音频顺序")}
                {orderedMedia(segment, form, "referenceVideoIds", "video", "参考视频顺序")}
                {segment.warnings.map((warning) => <small className="h3-local-import-warning" key={warning}>{warning}</small>)}
                <div className="h3-project-segment-actions">
                  <button type="button" onClick={() => onSave(segment)} disabled={busy}>保存本段修改</button>
                  <button type="button" className="quiet-button" onClick={() => onReset(segment)} disabled={busy}>恢复自动识别</button>
                  <span>模式来源：{segment.modeSource} · 分辨率来源：{projectParameterSourceLabel(segment.resolutionSource)} · 时长来源：{projectParameterSourceLabel(segment.durationSource)}</span>
                </div>
              </div>
            )}
          </article>
        );
      })}
    </div>
  );
}

export function AssetVideoBatchWorkspace({
  projectId,
  catalog,
  initialAssets,
  comfyConnected,
  taskEventsReady,
  productionAdmission,
  focusProductionBatchId,
  onAdmissionChanged,
  onProductionBatchFocused,
  onOpenTask,
  onBackToAssets,
  onOpenWorkflows,
}: Props) {
  const [sourceMode, setSourceMode] = useState<"ASSET_LIBRARY" | "LOCAL_FOLDER">("ASSET_LIBRARY");
  const [localInspection, setLocalInspection] = useState<H3LocalImportInspection>();
  const [projectSegmentForms, setProjectSegmentForms] = useState<Record<string, ProjectSegmentForm>>({});
  const [localBatchName, setLocalBatchName] = useState("");
  const [localAutoStart, setLocalAutoStart] = useState(true);
  const [expandedLocalOrdinal, setExpandedLocalOrdinal] = useState<number>();
  const [generationMode, setGenerationMode] = useState<H3GenerationMode>(() => h3InitialGenerationMode(initialAssets));
  const [qualityProfile, setQualityProfile] = useState<H3QualityProfile>(H3_QUALITY_PROFILE);
  const [manualVideoSelection, setManualVideoSelection] = useState<SelectedRecipeRef | undefined>(
    () => readSelectedRecipeRef(projectId, "video"),
  );
  const [projectWorkflowStrategy, setProjectWorkflowStrategy] = useState<"AUTO" | "MANUAL">("AUTO");
  const [projectManualOverrides, setProjectManualOverrides] = useState<Partial<Record<H3CompatibleMode, SelectedRecipeRef>>>(
    () => readProjectWorkflowOverrides(projectId),
  );
  const [workflowSelectionNotice, setWorkflowSelectionNotice] = useState<string>();
  const [batchPrompt, setBatchPrompt] = useState("");
  const [firstFrameAssetId, setFirstFrameAssetId] = useState<string>();
  const [lastFrameAssetId, setLastFrameAssetId] = useState<string>();
  const [availableAssets, setAvailableAssets] = useState<AssetView[]>(initialAssets);
  const [assetLibraryKeywordInput, setAssetLibraryKeywordInput] = useState("");
  const [assetLibraryKeyword, setAssetLibraryKeyword] = useState("");
  const [assetLibraryMediaType, setAssetLibraryMediaType] = useState<AssetMediaTypeFilter>("ALL");
  const [assetLibraryCursor, setAssetLibraryCursor] = useState<PageCursor>();
  const [assetLibraryLoading, setAssetLibraryLoading] = useState(false);
  const [assetLibraryError, setAssetLibraryError] = useState<string>();
  const videoCatalog = useMemo(() => filterVideoRecipes(catalog), [catalog]);
  const recommendedRecipe = useMemo(
    () => h3RecipeForMode(videoCatalog, generationMode, qualityProfile)
      ?? videoCatalog.find((candidate) => videoRecipeCapability(candidate).supportedModes.includes(generationMode)),
    [generationMode, qualityProfile, videoCatalog],
  );
  const resolvedVideoRecipe = useMemo(
    () => resolveVideoRecipe(videoCatalog, generationMode, manualVideoSelection, recommendedRecipe),
    [generationMode, manualVideoSelection, recommendedRecipe, videoCatalog],
  );
  const recipe = resolvedVideoRecipe.recipe;
  const projectModes = useMemo(
    () => [...new Set((localInspection?.projectFolder?.segments ?? []).map((segment) => segment.generationMode))] as H3CompatibleMode[],
    [localInspection?.projectFolder?.segments],
  );
  const projectRecommendations = useMemo(
    () => Object.fromEntries(projectModes.map((mode) => {
      const recommended = h3RecipeForMode(videoCatalog, mode, qualityProfile);
      return [mode, recommended ? recipeRef(recommended) : undefined];
    })) as Partial<Record<H3CompatibleMode, SelectedRecipeRef>>,
    [projectModes, qualityProfile, videoCatalog],
  );
  const resolvedProjectRecipes = useMemo(
    () => resolveProjectFolderRecipes(
      videoCatalog,
      projectModes,
      projectRecommendations,
      projectWorkflowStrategy === "MANUAL" ? projectManualOverrides : {},
    ),
    [projectManualOverrides, projectModes, projectRecommendations, projectWorkflowStrategy, videoCatalog],
  );
  const contract = useMemo(
    () => recipe
      ? h3RecipeContract(recipe)
      : { ok: false as const, reason: "运行目录中没有精确的 MiniMax H3 Recipe。" },
    [recipe],
  );
  const [prompts, setPrompts] = useState<Record<string, string>>({});
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set(initialAssets.map((asset) => asset.id)));
  const [savedIds, setSavedIds] = useState<Set<string>>(new Set());
  const [durationSeconds, setDurationSeconds] = useState<number>();
  const [width, setWidth] = useState<number>();
  const [height, setHeight] = useState<number>();
  const [createdBatchId, setCreatedBatchId] = useState<string>();
  const [createdBatchStarted, setCreatedBatchStarted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string>();
  const assetLibraryRequestVersion = useRef(0);
  const loadedPromptIds = useRef<Set<string>>(new Set());
  const selectedIdsRef = useRef(selectedIds);

  useEffect(() => {
    selectedIdsRef.current = selectedIds;
  }, [selectedIds]);

  useEffect(() => {
    const initialIds = new Set(initialAssets.map((asset) => asset.id));
    setAvailableAssets((current) => {
      if ([...initialIds].every((assetId) => current.some((asset) => asset.id === assetId))) return current;
      const byId = new Map(initialAssets.map((asset) => [asset.id, asset]));
      current.forEach((asset) => byId.set(asset.id, asset));
      return [...byId.values()];
    });
    setSelectedIds((current) => {
      if ([...initialIds].every((assetId) => current.has(assetId))) return current;
      const next = new Set(current);
      initialIds.forEach((assetId) => next.add(assetId));
      return next;
    });
    if (initialAssets.length === 1 && isImageAssetForVideo(initialAssets[0])) {
      setFirstFrameAssetId((current) => current ?? initialAssets[0].id);
    }
  }, [initialAssets]);

  useEffect(() => {
    setAvailableAssets(initialAssets);
    setSelectedIds(new Set(initialAssets.map((asset) => asset.id)));
    setFirstFrameAssetId(initialAssets.length === 1 && isImageAssetForVideo(initialAssets[0]) ? initialAssets[0].id : undefined);
    setLastFrameAssetId(undefined);
    setGenerationMode(h3InitialGenerationMode(initialAssets));
    setAssetLibraryCursor(undefined);
    setAssetLibraryError(undefined);
    setAssetLibraryKeywordInput("");
    setAssetLibraryKeyword("");
    setAssetLibraryMediaType("ALL");
    setPrompts({});
    setSavedIds(new Set());
    loadedPromptIds.current = new Set();
    setManualVideoSelection(readSelectedRecipeRef(projectId, "video"));
    setProjectWorkflowStrategy("AUTO");
    setProjectManualOverrides(readProjectWorkflowOverrides(projectId));
    setWorkflowSelectionNotice(undefined);
  }, [projectId]);

  useEffect(() => {
    if (!resolvedVideoRecipe.staleManualSelection || !manualVideoSelection) return;
    clearSelectedRecipeRef(projectId, "video");
    setManualVideoSelection(undefined);
    setWorkflowSelectionNotice("当前工作流不支持此生成模式，已切换到兼容工作流。");
  }, [manualVideoSelection, projectId, resolvedVideoRecipe.staleManualSelection]);

  useEffect(() => {
    const timer = window.setTimeout(() => setAssetLibraryKeyword(assetLibraryKeywordInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [assetLibraryKeywordInput]);

  const requestAssetPage = useCallback(async (requestedCursor: PageCursor | undefined, reset: boolean) => {
    const version = ++assetLibraryRequestVersion.current;
    setAssetLibraryLoading(true);
    setAssetLibraryError(undefined);
    try {
      const page = await assetLibraryPage({
        projectId,
        category: "ALL",
        keyword: assetLibraryKeyword || undefined,
        mediaType: assetLibraryMediaType,
        sourceKind: "ALL",
        createdOrder: "NEWEST",
        cursor: requestedCursor,
        limit: 30,
      });
      if (assetLibraryRequestVersion.current !== version) return;
      setAvailableAssets((current) => {
        const base = reset
          ? [...initialAssets, ...current.filter((asset) => selectedIdsRef.current.has(asset.id))]
          : current;
        const byId = new Map(base.map((asset) => [asset.id, asset]));
        page.items.forEach((asset) => byId.set(asset.id, asset));
        const merged = [...byId.values()];
        return !assetLibraryKeyword && assetLibraryMediaType === "ALL" && !page.nextCursor
          ? page.items
          : merged;
      });
      setAssetLibraryCursor(page.nextCursor);
      if (!page.nextCursor && !assetLibraryKeyword && assetLibraryMediaType === "ALL") {
        const pageIds = new Set(page.items.map((asset) => asset.id));
        setSelectedIds((current) => new Set([...current].filter((assetId) => pageIds.has(assetId))));
        setFirstFrameAssetId((current) => current && pageIds.has(current) ? current : undefined);
        setLastFrameAssetId((current) => current && pageIds.has(current) ? current : undefined);
      }
    } catch (error: unknown) {
      if (assetLibraryRequestVersion.current === version) setAssetLibraryError(toUserMessage(error));
    } finally {
      if (assetLibraryRequestVersion.current === version) setAssetLibraryLoading(false);
    }
  }, [assetLibraryKeyword, assetLibraryMediaType, initialAssets, projectId]);

  useEffect(() => {
    if (sourceMode !== "ASSET_LIBRARY") return () => undefined;
    setAssetLibraryCursor(undefined);
    void requestAssetPage(undefined, true);
    return () => {
      assetLibraryRequestVersion.current += 1;
    };
  }, [requestAssetPage, sourceMode]);

  useEffect(() => {
    const imageIds = availableAssets.filter(isImageAssetForVideo).map((asset) => asset.id);
    const pendingIds = imageIds.filter((assetId) => !loadedPromptIds.current.has(assetId));
    if (!pendingIds.length) return () => undefined;
    pendingIds.forEach((assetId) => loadedPromptIds.current.add(assetId));
    let active = true;
    void listAssetVideoPrompts(projectId, pendingIds)
      .then((records) => {
        if (!active) return;
        setPrompts((current) => ({
          ...current,
          ...Object.fromEntries(records.map((record) => [record.assetId, record.promptText])),
        }));
        setSavedIds((current) => new Set([...current, ...records.map((record) => record.assetId)]));
      })
      .catch((error: unknown) => {
        pendingIds.forEach((assetId) => loadedPromptIds.current.delete(assetId));
        if (active) setNotice(toUserMessage(error));
      });
    return () => { active = false; };
  }, [availableAssets, projectId]);

  useEffect(() => {
    setDurationSeconds(contract.ok ? contract.contract.durationField.default : undefined);
    setWidth(contract.ok ? contract.contract.widthField.default : undefined);
    setHeight(contract.ok ? contract.contract.heightField.default : undefined);
  }, [contract]);

  const selectedAssets = [...selectedIds]
    .map((assetId) => availableAssets.find((asset) => asset.id === assetId))
    .filter((asset): asset is AssetView => Boolean(asset));
  const imageAssets = selectedAssets.filter(isImageAssetForVideo);
  const videoAssets = selectedAssets.filter(isVideoAssetForH3);
  const audioAssets = selectedAssets.filter(isAudioAssetForH3);
  const missingPromptAssets = imageAssets.filter((asset) => !prompts[asset.id]?.trim());
  const selectedDuration = contract.ok ? durationSeconds ?? contract.contract.durationField.default : undefined;
  const selectedWidth = contract.ok ? width ?? contract.contract.widthField.default : undefined;
  const selectedHeight = contract.ok ? height ?? contract.contract.heightField.default : undefined;
  const durationReady = Boolean(
    contract.ok
      && selectedDuration !== undefined
      && contract.contract.durationOptions.includes(selectedDuration),
  );
  const resolutionValidation = recipe && contract.ok
    ? validateResolution(recipe, selectedWidth, selectedHeight)
    : undefined;
  const resolutionReady = Boolean(
    resolutionValidation?.ok
      && selectedWidth !== undefined
      && selectedHeight !== undefined
      && isMinimaxH3OutputResolution(selectedWidth, selectedHeight),
  );
  const runtimeReady = Boolean(contract.ok && comfyConnected && taskEventsReady && durationReady && resolutionReady);
  const resolutionPresets = contract.ok && recipe
    ? resolutionPresetsForRecipe(recipe, MINIMAX_H3_RESOLUTION_PRESETS)
    : [];
  const exceedsHistoricalProfile = Boolean(
    selectedDuration !== undefined
      && selectedWidth !== undefined
      && selectedHeight !== undefined
      && (selectedDuration > 5 || selectedWidth * selectedHeight > 100_000),
  );
  const modeSupported = Boolean(contract.ok && h3ModeSupported(contract.contract, generationMode));
  const modePrompt = batchPrompt.trim() || prompts[firstFrameAssetId ?? imageAssets[0]?.id] || "";
  const modePromptTooLong = new TextEncoder().encode(modePrompt).byteLength > 64 * 1024;
  const modeAssetReady = (() => {
    switch (generationMode) {
      case "FL2VA_TEXT_TO_VIDEO": return true;
      case "FL2VA_IMAGE_TO_VIDEO": return Boolean(firstFrameAssetId && imageAssets.some((asset) => asset.id === firstFrameAssetId));
      case "FL2VA_FIRST_LAST": return Boolean(
        firstFrameAssetId
          && lastFrameAssetId
          && firstFrameAssetId !== lastFrameAssetId
          && imageAssets.some((asset) => asset.id === firstFrameAssetId)
          && imageAssets.some((asset) => asset.id === lastFrameAssetId),
      );
      case "REF2VA_IMAGE": return imageAssets.length > 0;
      case "REF2VA_AUDIO": return audioAssets.length > 0;
      case "REF2VA_IMAGE_AUDIO": return imageAssets.length > 0 && audioAssets.length > 0;
      case "REF2VA_VIDEO_IMAGE": return videoAssets.length > 0 && imageAssets.length > 0;
    }
  })();
  const canCreate = runtimeReady
    && !productionAdmission.busy
    && modeSupported
    && modeAssetReady
    && modePrompt.length > 0
    && !modePromptTooLong
    && (generationMode.startsWith("FL2VA") ? selectedAssets.length <= 100 : selectedAssets.length <= 100);
  const projectRecipesReady = Boolean(
    localInspection?.projectFolder
      && localInspection.projectFolder.segments.every((segment) => (
        resolvedProjectRecipes.some((resolved) => (
          resolved.mode === segment.generationMode
          && Boolean(resolved.recipe)
          && (projectWorkflowStrategy === "AUTO" || !resolved.staleManualSelection)
        ))
      )),
  );
  const localCanCreate = localImportCanCommit(
    localInspection,
    localInspection?.mode === "PROJECT_FOLDER"
      ? Boolean(comfyConnected && taskEventsReady && projectRecipesReady)
      : runtimeReady,
    productionAdmission.busy,
  );
  const batchDraft = useMemo(
    () => buildH3BatchDraft({
      recipe,
      contract,
      mode: generationMode,
      prompt: modePrompt,
      promptTooLong: modePromptTooLong,
      durationSeconds: selectedDuration,
      width: selectedWidth,
      height: selectedHeight,
      durationReady,
      resolutionReady,
      modeSupported,
      modeAssetReady,
      firstFrameAssetId,
      lastFrameAssetId,
      imageAssetIds: imageAssets.map((asset) => asset.id),
      videoAssetIds: videoAssets.map((asset) => asset.id),
      audioAssetIds: audioAssets.map((asset) => asset.id),
    }),
    [
      audioAssets,
      contract,
      durationReady,
      firstFrameAssetId,
      generationMode,
      imageAssets,
      lastFrameAssetId,
      modeAssetReady,
      modePrompt,
      modePromptTooLong,
      modeSupported,
      recipe,
      resolutionReady,
      selectedDuration,
      selectedHeight,
      selectedWidth,
      videoAssets,
    ],
  );
  const batchItems: BatchDraftItem[] = batchDraft.items;

  function addSelectedAsset(assetId: string) {
    setSelectedIds((current) => {
      if (current.has(assetId) || current.size >= 100) return current;
      return new Set([...current, assetId]);
    });
  }

  function setFirstFrame(asset: AssetView) {
    if (!isImageAssetForVideo(asset)) return;
    setFirstFrameAssetId(asset.id);
    if (generationMode === "FL2VA_IMAGE_TO_VIDEO") setSelectedIds(new Set([asset.id]));
    else addSelectedAsset(asset.id);
    if (lastFrameAssetId === asset.id) setLastFrameAssetId(undefined);
  }

  function setLastFrame(asset: AssetView) {
    if (!isImageAssetForVideo(asset) || firstFrameAssetId === asset.id) return;
    setLastFrameAssetId(asset.id);
    addSelectedAsset(asset.id);
  }

  function toggleAsset(asset: AssetView) {
    if (!h3PickerAssets([asset], generationMode).length) return;
    if (generationMode === "FL2VA_IMAGE_TO_VIDEO") {
      setFirstFrame(asset);
      return;
    }
    if (generationMode === "FL2VA_FIRST_LAST") {
      if (!selectedIds.has(asset.id)) {
        addSelectedAsset(asset.id);
        if (!firstFrameAssetId) setFirstFrame(asset);
        else if (!lastFrameAssetId && firstFrameAssetId !== asset.id) setLastFrame(asset);
      } else {
        setSelectedIds((current) => new Set([...current].filter((assetId) => assetId !== asset.id)));
        if (firstFrameAssetId === asset.id) setFirstFrameAssetId(undefined);
        if (lastFrameAssetId === asset.id) setLastFrameAssetId(undefined);
      }
      return;
    }
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(asset.id)) next.delete(asset.id);
      else if (next.size < 100) next.add(asset.id);
      return next;
    });
  }

  function moveSelectedAsset(assetId: string, delta: number) {
    setSelectedIds((current) => {
      const ids = [...current];
      const targetAsset = availableAssets.find((asset) => asset.id === assetId);
      if (!targetAsset) return current;
      const selected = ids
        .map((id) => availableAssets.find((asset) => asset.id === id))
        .filter((asset): asset is AssetView => Boolean(asset))
        .filter((asset) => asset.assetType === targetAsset.assetType);
      const orderedAssets = h3PickerAssets(selected, generationMode);
      const orderedIds = orderedAssets.map((asset) => asset.id);
      const index = orderedIds.indexOf(assetId);
      const target = index + delta;
      if (index < 0 || target < 0 || target >= orderedIds.length) return current;
      [orderedIds[index], orderedIds[target]] = [orderedIds[target], orderedIds[index]];
      let orderedIndex = 0;
      const nextIds = ids.map((id) => orderedIds.includes(id) ? orderedIds[orderedIndex++] : id);
      return new Set(nextIds);
    });
  }

  function removeSelectedAsset(assetId: string) {
    setSelectedIds((current) => new Set([...current].filter((id) => id !== assetId)));
    if (firstFrameAssetId === assetId) setFirstFrameAssetId(undefined);
    if (lastFrameAssetId === assetId) setLastFrameAssetId(undefined);
  }

  function updatePrompt(assetId: string, value: string) {
    setPrompts((current) => ({ ...current, [assetId]: value }));
    setSavedIds((current) => {
      if (!current.has(assetId)) return current;
      const next = new Set(current);
      next.delete(assetId);
      return next;
    });
  }

  async function savePrompt(asset: AssetView) {
    setBusy(true); setNotice(undefined);
    try {
      await setAssetVideoPrompt(projectId, asset.id, prompts[asset.id] ?? "");
      setSavedIds((current) => new Set(current).add(asset.id));
      setNotice(`已保存「${asset.name}」的视频提示词。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function createBatch() {
    if (!recipe || !contract.ok || selectedDuration === undefined || !canCreate) return;
    setBusy(true); setNotice(undefined);
    try {
      if (generationMode.startsWith("FL2VA")) {
        const promptAssets = imageAssets.filter((asset) => prompts[asset.id]?.trim());
        await Promise.all(promptAssets.map((asset) => setAssetVideoPrompt(projectId, asset.id, prompts[asset.id] ?? "")));
        setSavedIds(new Set(promptAssets.map((asset) => asset.id)));
      }
      const created = await createProductionQueue({
        projectId,
        name: `批量视频 · ${new Date().toLocaleString()}`,
        continueOnFailure: true,
        items: batchItems.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          values: item.values,
        })),
      });
      setCreatedBatchId(created.id);
      setCreatedBatchStarted(false);
      await onAdmissionChanged();
      try {
        await startProductionQueue(projectId, created.id);
        setCreatedBatchStarted(true);
        setNotice(`已创建并开始视频批次，共 ${created.total} 项；参数和视频提示词已冻结。`);
      } catch (startError: unknown) {
        setNotice(`已创建视频批次，共 ${created.total} 项；开始执行失败：${toUserMessage(startError)}。可手动开始。`);
      }
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function startBatch() {
    if (!createdBatchId) return;
    setBusy(true); setNotice(undefined);
    try {
      await startProductionQueue(projectId, createdBatchId);
      setCreatedBatchStarted(true);
      setNotice("视频批次已开始，队列将严格串行执行。");
      await onAdmissionChanged();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function syncProjectSegmentForms(inspection: H3LocalImportInspection) {
    const segments = inspection.projectFolder?.segments ?? [];
    setProjectSegmentForms(Object.fromEntries(segments.map((segment) => [segment.segmentId, projectSegmentForm(segment)])));
  }

  function applyLocalInspection(inspection: H3LocalImportInspection) {
    setLocalInspection(inspection);
    syncProjectSegmentForms(inspection);
  }

  function updateProjectSegmentForm(segmentId: string, patch: Partial<ProjectSegmentForm>) {
    setProjectSegmentForms((current) => {
      const existing = current[segmentId];
      if (!existing) return current;
      return { ...current, [segmentId]: { ...existing, ...patch } };
    });
  }

  async function saveProjectSegment(segment: H3ProjectSegment) {
    if (!localInspection?.projectFolder) return;
    const form = projectSegmentForms[segment.segmentId] ?? projectSegmentForm(segment);
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await updateH3ProjectSegmentDraft({
        sessionId: localInspection.sessionId,
        segmentId: segment.segmentId,
        mode: form.mode,
        prompt: form.prompt,
        durationSeconds: form.durationSeconds,
        width: form.width,
        height: form.height,
        referenceImageIds: form.referenceImageIds,
        referenceAudioIds: form.referenceAudioIds,
        referenceVideoIds: form.referenceVideoIds,
        firstFrameId: form.firstFrameId,
        lastFrameId: form.lastFrameId,
      });
      applyLocalInspection(inspection);
      setNotice(`已保存第 ${segment.ordinal} 段编辑，提交时将冻结本段参数。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function resetProjectSegment(segment: H3ProjectSegment) {
    if (!localInspection?.projectFolder) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await updateH3ProjectSegmentDraft({
        sessionId: localInspection.sessionId,
        segmentId: segment.segmentId,
        resetAutoDetection: true,
      });
      applyLocalInspection(inspection);
      setNotice(`第 ${segment.ordinal} 段已恢复自动识别。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function updateProjectWorkflowOverride(mode: H3CompatibleMode, ref: SelectedRecipeRef | undefined) {
    setProjectManualOverrides((current) => {
      const next = { ...current };
      if (ref) next[mode] = ref;
      else delete next[mode];
      writeProjectWorkflowOverrides(projectId, next);
      return next;
    });
  }

  async function chooseLocalDirectory() {
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await pickH3LocalImportDirectory(projectId, "PROJECT_FOLDER");
      if (!inspection) return;
      applyLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
      setNotice(`已读取「${inspection.displayRootName}」，可生成 ${inspection.readyCount} 项。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function rescanLocalDirectory() {
    if (!localInspection) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await rescanH3LocalImport(localInspection.sessionId, "PROJECT_FOLDER");
      applyLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
      setNotice(`已重新扫描，当前可生成 ${inspection.readyCount} 项。`);
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setProjectSegmentForms({});
      setExpandedLocalOrdinal(undefined);
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function commitLocalBatch() {
    if (!recipe || !contract.ok || !localInspection || !localCanCreate) return;
    const selectedDuration = durationSeconds ?? contract.contract.durationField.default;
    const selectedWidth = width ?? contract.contract.widthField.default;
    const selectedHeight = height ?? contract.contract.heightField.default;
    if (selectedDuration === undefined || selectedWidth === undefined || selectedHeight === undefined) return;
    setBusy(true); setNotice(undefined);
    try {
      const qualityRecipes = resolvedProjectRecipes
        .flatMap((resolved) => resolved.recipe
          ? [{ mode: resolved.mode, workflowVersionId: resolved.recipe.workflowVersionId, recipeId: resolved.recipe.recipeId }]
          : [])
      const result = await commitH3LocalImport({
        sessionId: localInspection.sessionId,
        batchName: localBatchName.trim() || undefined,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        width: selectedWidth,
        height: selectedHeight,
        durationSeconds: selectedDuration,
        autoStart: localAutoStart,
        generationMode,
        fl2vaWorkflowVersionId: catalog.find((item) => item.workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID && item.outputTypes?.includes("video"))?.workflowVersionId,
        fl2vaRecipeId: catalog.find((item) => item.workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID && item.outputTypes?.includes("video"))?.recipeId,
        ref2vaWorkflowVersionId: catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video"))?.workflowVersionId,
        ref2vaRecipeId: catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video"))?.recipeId,
        qualityProfile,
        qualityRecipes,
      });
      setCreatedBatchId(result.batchId);
      setCreatedBatchStarted(result.autoStarted);
      setLocalInspection(undefined);
      setProjectSegmentForms({});
      setExpandedLocalOrdinal(undefined);
      await onAdmissionChanged();
      setNotice(
        `本地任务已导入，共${result.itemCount}项。${result.autoStarted ? "已开始生成；" : "已创建批次；"}素材已进入资产库。${result.warnings.length ? ` ${result.warnings.join("；")}` : ""}`,
      );
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setProjectSegmentForms({});
      setExpandedLocalOrdinal(undefined);
      setNotice(`导入未完成，请重新选择项目文件夹。${toUserMessage(error)}`);
    } finally {
      setBusy(false);
    }
  }

  function selectVideoWorkflow(nextRecipe: RecipeViewModel) {
    const nextRef = recipeRef(nextRecipe);
    writeSelectedRecipeRef(projectId, "video", nextRef);
    setManualVideoSelection(nextRef);
    setWorkflowSelectionNotice(undefined);
  }

  function restoreRecommendedVideoWorkflow() {
    if (!recommendedRecipe) return;
    clearSelectedRecipeRef(projectId, "video");
    setManualVideoSelection(undefined);
    setWorkflowSelectionNotice(undefined);
  }

  if (recipe && !contract.ok) {
    return (
      <section className="workspace-panel asset-video-batch-workspace" aria-busy={busy}>
        <div className="section-heading workspace-heading">
          <div>
            <span className="section-label">视频工作流</span>
            <h2>{recipe.name}</h2>
            <p className="section-description">该工作流使用通用参数模式，不会强行套用 MiniMax H3 的模式、质量或项目文件夹参数。</p>
          </div>
          <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
        </div>
        <WorkflowSelector
          stage="video"
          candidates={videoWorkflowCandidatesForMode(videoCatalog, generationMode)}
          selected={recipe}
          recommended={recommendedRecipe}
          selectionSource={resolvedVideoRecipe.source}
          onSelect={selectVideoWorkflow}
          onRestoreRecommendation={restoreRecommendedVideoWorkflow}
          onOpenWorkflows={onOpenWorkflows ? () => onOpenWorkflows() : undefined}
        />
        {workflowSelectionNotice && <p className="workflow-selection-notice" role="status">{workflowSelectionNotice}</p>}
        <GenericVideoWorkflowPanel
          projectId={projectId}
          recipe={recipe}
          comfyConnected={comfyConnected}
          taskEventsReady={taskEventsReady}
          productionBusy={productionAdmission.busy}
          onOpenTask={onOpenTask}
        />
      </section>
    );
  }

  if (!recipe && !videoCatalog.length) {
    return (
      <section className="workspace-panel asset-video-batch-workspace" aria-label="视频工作流为空">
        <div className="section-heading workspace-heading">
          <div>
            <span className="section-label">视频工作流</span>
            <h2>没有可用视频工作流</h2>
            <p className="section-description">请先导入并发布一个声明视频输出的工作流。</p>
          </div>
          <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
        </div>
        <p className="disabled-note">没有可用视频工作流。</p>
        {onOpenWorkflows && <button type="button" onClick={onOpenWorkflows}>导入工作流</button>}
      </section>
    );
  }

  return (
    <section className="workspace-panel asset-video-batch-workspace" aria-busy={busy}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">批量视频</span>
          <h2>资产 + 视频提示词</h2>
          <p className="section-description">从当前项目图片资产直接创建 MiniMax H3 视频批次；它与图片批次相互独立。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
      </div>

      <WorkflowSelector
        stage="video"
        candidates={videoWorkflowCandidatesForMode(videoCatalog, generationMode)}
        selected={recipe}
        recommended={recommendedRecipe}
        selectionSource={resolvedVideoRecipe.source}
        onSelect={selectVideoWorkflow}
        onRestoreRecommendation={restoreRecommendedVideoWorkflow}
        onOpenWorkflows={onOpenWorkflows ? () => onOpenWorkflows() : undefined}
      />
      {workflowSelectionNotice && <p className="workflow-selection-notice" role="status">{workflowSelectionNotice}</p>}

      <div className="asset-video-source-tabs" role="tablist" aria-label="视频批量输入来源">
        <button
          type="button"
          role="tab"
          aria-selected={sourceMode === "ASSET_LIBRARY"}
          className={sourceMode === "ASSET_LIBRARY" ? "asset-video-source-tab asset-video-source-tab-active" : "asset-video-source-tab"}
          onClick={() => setSourceMode("ASSET_LIBRARY")}
          disabled={busy}
        >
          从资产库
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={sourceMode === "LOCAL_FOLDER"}
          className={sourceMode === "LOCAL_FOLDER" ? "asset-video-source-tab asset-video-source-tab-active" : "asset-video-source-tab"}
          onClick={() => setSourceMode("LOCAL_FOLDER")}
          disabled={busy}
        >
          从本地导入
        </button>
      </div>

      {sourceMode === "ASSET_LIBRARY" && (
      <section className="h3-mode-selector" aria-label="MiniMax H3 生成模式">
        <div className="h3-mode-selector-heading">
          <div>
            <span className="section-label">H3 Full Mode</span>
            <h3>选择生成类型</h3>
            <p className="section-description">只显示本机已安装并通过节点契约审计的模式；未满足 graph 条件的模式会明确禁用。</p>
          </div>
          <span className="h3-mode-contract-badge">{recipe ? `${recipe.name} · ${recipe.mode}` : "未找到可用 Recipe"}</span>
        </div>
        <div className="h3-mode-family-tabs" role="tablist" aria-label="H3 模式家族">
          {(["FL2VA", "REF2VA"] as const).map((family) => {
            const familyMode = H3_MODE_OPTIONS.find((option) => option.family === family)?.id;
            const active = generationMode.startsWith(family);
            const available = Boolean(familyMode && videoCatalog.some((candidate) => (
              videoRecipeCapability(candidate).supportedModes.includes(familyMode)
              && h3RecipeContract(candidate).ok
            )));
            return (
              <button
                key={family}
                type="button"
                role="tab"
                aria-selected={active}
                className={active ? "h3-mode-family-tab h3-mode-family-tab-active" : "h3-mode-family-tab"}
                onClick={() => familyMode && setGenerationMode(familyMode)}
                disabled={busy || !available}
                title={!available ? "当前本地 H3 工作流未启用该模式" : undefined}
              >
                {family === "FL2VA" ? "文生 / 图生视频" : "全能参考"}
              </button>
            );
          })}
        </div>
        <div className="h3-mode-options" role="listbox" aria-label="H3 具体模式">
          {H3_MODE_OPTIONS.filter((option) => option.family === (generationMode.startsWith("FL2VA") ? "FL2VA" : "REF2VA")).map((option) => {
            const optionRecipe = h3RecipeForMode(videoCatalog, option.id, qualityProfile);
            const optionContract = optionRecipe ? h3RecipeContract(optionRecipe) : undefined;
            const available = Boolean(optionContract?.ok && h3ModeSupported(optionContract.contract, option.id));
            return (
              <button
                key={option.id}
                type="button"
                role="option"
                aria-selected={generationMode === option.id}
                className={generationMode === option.id ? "h3-mode-option h3-mode-option-active" : "h3-mode-option"}
                onClick={() => setGenerationMode(option.id)}
                disabled={busy || !available}
                title={!available ? "当前本地 H3 工作流未启用该模式" : option.description}
              >
                <strong>{option.label}</strong>
                <span>{available ? option.description : "当前本地 H3 工作流未启用该模式"}</span>
              </button>
            );
          })}
        </div>
        {!modeSupported && <p className="h3-mode-disabled-note" role="status">当前本地 H3 工作流未启用该模式。请先安装/启用经过本机 `/object_info` 与 graph 审计的 Recipe。</p>}
      </section>
      )}

      <section className="h3-quality-profile" aria-label="H3 生成质量">
        <label htmlFor="h3-quality-profile">生成质量</label>
        <select
          id="h3-quality-profile"
          value={qualityProfile}
          onChange={(event) => setQualityProfile(event.target.value as H3QualityProfile)}
          disabled={busy}
        >
          <option value="QUALITY">高质量（推荐）</option>
          <option value="FAST">快速预览</option>
        </select>
        <small>
          {qualityProfile === "QUALITY"
            ? "高质量：20步正式工作流，生成更慢，显存和内存占用更高。"
            : "快速预览：4步 Turbo，速度优先，画质和参考一致性可能低于高质量模式。"}
        </small>
      </section>

      <section className="h3-safety-card" aria-label="H3 安全配置">
        <div>
          <strong>MiniMax H3</strong>
          <p>模型产品能力：最高 15 秒 · 最高 2K</p>
          <p>当前 Runtime：{qualityProfile === "QUALITY" ? "20 步正式工作流" : "4 步 Turbo 预览"} · 单任务串行</p>
          <small>{qualityProfile === "QUALITY" ? "QUALITY 不会因 16GB 设备自动降级；失败按正常 Task FAILED 处理。" : "FAST 仅用于快速预览，历史包保持不变。"}</small>
        </div>
        {sourceMode === "ASSET_LIBRARY" && contract.ok && (
          <ResolutionControl
            widthField={contract.contract.widthField}
            heightField={contract.contract.heightField}
            width={selectedWidth}
            height={selectedHeight}
            presets={resolutionPresets}
            presetsOnly
            disabled={busy}
            onChange={(next) => {
              setWidth(next.width);
              setHeight(next.height);
            }}
          />
        )}
        {sourceMode === "ASSET_LIBRARY" && <div className="h3-duration-control">
          <label htmlFor="h3-duration-seconds">视频时长</label>
          <select
            id="h3-duration-seconds"
            value={selectedDuration ?? ""}
            onChange={(event) => setDurationSeconds(Number(event.target.value))}
            disabled={busy || !contract.ok}
          >
            <option value="" disabled>Recipe 不可用</option>
            {contract.ok && contract.contract.durationOptions.map((option) => (
              <option key={option} value={option}>{option} 秒</option>
            ))}
          </select>
          <small>
            {contract.ok
              ? `Recipe 范围 ${contract.contract.durationField.min}–${contract.contract.durationField.max} 秒 · 默认 ${contract.contract.durationField.default} 秒`
              : "H3 runtime unavailable"}
          </small>
        </div>}
        {sourceMode === "LOCAL_FOLDER" && (
          <div className="h3-project-folder-defaults" role="status">
            <strong>Segment 参数优先级</strong>
            <span>用户修改 → Front Matter → 提示词规格 → 素材比例 → 默认</span>
            <small>无规格且无可用素材推断时使用 {qualityProfile} · 5 秒 · 960 × 544；每段可展开单独编辑。</small>
          </div>
        )}
        <small>{recipe ? `运行时已锁定：${recipe.workflowId}` : "运行时未就绪"}</small>
      </section>
      {sourceMode === "ASSET_LIBRARY" && exceedsHistoricalProfile && (
        <p className="h3-profile-warning" role="status">
          当前配置超出本机已验证范围。模型/产品允许该配置，但 RTX 5060 Ti 16GB 的显存占用尚未验证，生成时可能出现显存不足。
        </p>
      )}

      {sourceMode === "LOCAL_FOLDER" ? (
        <div className="h3-local-import-layout">
          <section className="h3-local-import-panel" aria-label="MiniMax H3 项目文件夹批量导入">
            <div className="section-heading">
              <div>
                <span className="section-label">本地批量</span>
                <h3>项目文件夹导入</h3>
                <p className="section-description">选择一个项目根目录；每个一级子文件夹对应一个视频 Segment。扫描只读，提交后才导入素材并创建严格串行队列。</p>
              </div>
              <button type="button" onClick={() => void chooseLocalDirectory()} disabled={busy}>
                {busy ? "处理中…" : "选择项目文件夹"}
              </button>
            </div>

            <ProjectFolderImportControls
              busy={busy}
              hasInspection={Boolean(localInspection)}
              onRescan={() => void rescanLocalDirectory()}
            />

            {!localInspection ? (
              <div className="h3-local-import-empty">
                <strong>尚未选择项目文件夹</strong>
                <span>项目根目录只在本机短时使用；页面不会显示或保存绝对路径。</span>
              </div>
            ) : (
              <>
                <div className="h3-local-import-root">
                  <span>当前目录</span>
                  <strong>{localInspection.displayRootName}</strong>
                </div>
                <div className="h3-local-import-summary" aria-label="本地批量扫描结果">
                  <span>图片 <strong>{localInspection.imageCount}</strong></span>
                  <span>Prompt <strong>{localInspection.promptCount}</strong></span>
                  <span className="h3-local-import-summary-ready">可生成 <strong>{localInspection.readyCount}</strong></span>
                  <span className={localInspection.errorCount ? "h3-local-import-summary-error" : ""}>异常 <strong>{localInspection.errorCount}</strong></span>
                </div>
                {localInspection.warnings.map((warning) => <p key={warning} className="h3-local-import-warning" role="status">{warning}</p>)}
                {localInspection.errors.length > 0 && (
                  <div className="h3-local-import-errors" role="alert">
                    <strong>导入前需要修复：</strong>
                    <ul>{localInspection.errors.slice(0, 12).map((error) => <li key={error}>{error}</li>)}</ul>
                    {localInspection.errors.length > 12 && <small>还有 {localInspection.errors.length - 12} 项异常，请修复后重新扫描。</small>}
                  </div>
                )}
                {localInspection.mode === "PROJECT_FOLDER" && localInspection.projectFolder ? (
                  <>
                    <ProjectFolderWorkflowStrategy
                      strategy={projectWorkflowStrategy}
                      modes={projectModes}
                      catalog={videoCatalog}
                      recommendations={projectRecommendations}
                      overrides={projectManualOverrides}
                      resolved={resolvedProjectRecipes}
                      busy={busy}
                      onStrategyChange={setProjectWorkflowStrategy}
                      onOverrideChange={updateProjectWorkflowOverride}
                    />
                    {projectWorkflowStrategy === "MANUAL" && resolvedProjectRecipes.some((item) => item.staleManualSelection) && (
                      <p className="error-message" role="alert">有手动指定的工作流已停用或不再兼容当前模式，请重新选择后再提交；不会静默改用其他工作流。</p>
                    )}
                    <ProjectFolderSegmentEditor
                      project={localInspection.projectFolder}
                      forms={projectSegmentForms}
                      busy={busy}
                      expandedOrdinal={expandedLocalOrdinal}
                      onToggle={(ordinal) => setExpandedLocalOrdinal(expandedLocalOrdinal === ordinal ? undefined : ordinal)}
                      onChange={updateProjectSegmentForm}
                      onSave={(segment) => void saveProjectSegment(segment)}
                      onReset={(segment) => void resetProjectSegment(segment)}
                    />
                  </>
                ) : (
                  <div className="h3-local-import-list" role="table" aria-label="本地批量项目列表">
                    <div className="h3-local-import-list-heading" role="row">
                      <span>序号</span><span>图片</span><span>提示词</span><span>字节</span><span>状态</span>
                    </div>
                    {localInspection.pairs.map((pair) => {
                      const expanded = expandedLocalOrdinal === pair.ordinal;
                      return (
                        <div className={`h3-local-import-row${pair.status === "READY" ? " h3-local-import-row-ready" : " h3-local-import-row-error"}`} role="row" key={`${pair.ordinal}-${pair.imageDisplayName}`}>
                          <span className="h3-local-import-ordinal">#{pair.ordinal}</span>
                          <span className="h3-local-import-file" title={pair.imageDisplayName}>
                            <span>{pair.imageDisplayName}</span>
                            {pair.lastImageDisplayName && <small>末帧：{pair.lastImageDisplayName}</small>}
                            {pair.videoDisplayNames?.map((name) => <small key={name}>视频：{name}</small>)}
                            {pair.audioDisplayNames?.map((name) => <small key={name}>音频：{name}</small>)}
                          </span>
                          <span className="h3-local-import-prompt">
                            <span className={expanded ? "" : "h3-local-import-prompt-preview"}>{pair.promptPreview ?? pair.promptDisplayName}</span>
                            {pair.promptPreview && <button type="button" className="quiet-button h3-local-import-prompt-toggle" onClick={() => setExpandedLocalOrdinal(expanded ? undefined : pair.ordinal)}>{expanded ? "收起" : "查看 Prompt"}</button>}
                          </span>
                          <span className="h3-local-import-bytes">{formatPromptBytes(pair.promptBytes)}</span>
                          <span className={pair.status === "READY" ? "h3-local-import-status h3-local-import-status-ready" : "h3-local-import-status"}>{localImportStatusLabel(pair.status)}</span>
                        </div>
                      );
                    })}
                  </div>
                )}
                <div className="h3-local-import-options">
                  <label>
                    <span>批次名称（可选）</span>
                    <input value={localBatchName} onChange={(event) => setLocalBatchName(event.target.value)} maxLength={120} placeholder="H3 本地批量" disabled={busy} />
                  </label>
                  <label className="h3-local-import-checkbox">
                    <input type="checkbox" checked={localAutoStart} onChange={(event) => setLocalAutoStart(event.target.checked)} disabled={busy} />
                    <span>导入后立即开始生成</span>
                  </label>
                </div>
                <div className="asset-video-batch-actions">
                  <button type="button" onClick={() => void commitLocalBatch()} disabled={busy || !localCanCreate}>
                    {busy ? "正在导入…" : localAutoStart ? `导入并生成（${localInspection.readyCount}）` : `导入并创建批次（${localInspection.readyCount}）`}
                  </button>
                  {createdBatchId && !createdBatchStarted && <button type="button" className="quiet-button" onClick={() => void startBatch()} disabled={busy || productionAdmission.busy}>开始生成</button>}
                </div>
                {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择图片规格中的 16:9 输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
              </>
            )}
          </section>
          <ProductionQueuePanel
            projectId={projectId}
            batchItems={[]}
            comfyConnected={comfyConnected}
            variant="inline"
            focusBatchId={createdBatchId
              ?? focusProductionBatchId
              ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
            onAdmissionChanged={onAdmissionChanged}
            onFocusedBatchOpened={() => {
              if (focusProductionBatchId) onProductionBatchFocused();
            }}
            onOpenTask={onOpenTask}
            hideCreate
          />
        </div>
      ) : (
        <div className="batch-workspace-grid">
          <div className="batch-editor-column">
          <H3AssetLibraryPicker
            projectId={projectId}
            assets={availableAssets}
            generationMode={generationMode}
            selectedIds={selectedIds}
            firstFrameAssetId={firstFrameAssetId}
            lastFrameAssetId={lastFrameAssetId}
            keyword={assetLibraryKeywordInput}
            mediaType={assetLibraryMediaType}
            loading={assetLibraryLoading}
            error={assetLibraryError}
            hasMore={Boolean(assetLibraryCursor)}
            busy={busy}
            onKeywordChange={setAssetLibraryKeywordInput}
            onMediaTypeChange={setAssetLibraryMediaType}
            onRefresh={() => void requestAssetPage(undefined, true)}
            onLoadMore={() => void requestAssetPage(assetLibraryCursor, false)}
            onToggleAsset={toggleAsset}
            onSetFirstFrame={setFirstFrame}
            onSetLastFrame={setLastFrame}
          />
          <section className="h3-batch-prompt-panel" aria-label="H3 批次提示词">
            <label htmlFor="h3-batch-prompt"><span>视频 Prompt</span><textarea id="h3-batch-prompt" value={batchPrompt} onChange={(event) => setBatchPrompt(event.target.value)} rows={5} maxLength={64 * 1024} disabled={busy || !modeSupported} placeholder="描述运动、镜头、声音和连续性；可使用 <Picture 1>、<Audio 1>、<Video 1>。" /></label>
            <small>{modePromptTooLong ? "Prompt 超过 64 KiB" : "该 Prompt 会随本次队列创建冻结。"}</small>
          </section>
          <div className="asset-video-batch-summary">
            <span>模式 <strong>{H3_MODE_OPTIONS.find((option) => option.id === generationMode)?.label}</strong></span>
            <span>已选择 <strong>{selectedAssets.length}</strong> 项</span>
            <span>图片 {imageAssets.length} · 视频 {videoAssets.length} · 音频 {audioAssets.length}</span>
            <span>队列项 <strong>{batchItems.length}</strong></span>
          </div>
          {generationMode.startsWith("REF2VA") && (
            <div className="h3-selected-reference-list" aria-label="已选择的参考素材">
              <div className="h3-selected-reference-heading">
                <strong>已选择参考素材</strong>
                <span>按列表顺序写入批次</span>
              </div>
              {([
                ["图片参考", selectedAssets.filter(isImageAssetForVideo)],
                ["音频参考", selectedAssets.filter(isAudioAssetForH3)],
                ["视频参考", selectedAssets.filter(isVideoAssetForH3)],
              ] as Array<[string, AssetView[]]>).filter(([, items]) => items.length > 0).map(([label, items]) => (
                <div className="h3-selected-reference-group" key={label}>
                  <strong>{label}</strong>
                  {items.map((asset, index) => (
                    <div className="h3-selected-reference-row" key={asset.id}>
                      <span className="h3-selected-reference-index">#{index + 1}</span>
                      <span className="h3-selected-reference-name">{asset.name}</span>
                      <button type="button" className="quiet-button" onClick={() => moveSelectedAsset(asset.id, -1)} disabled={busy || index === 0} aria-label={`上移${asset.name}`}>↑</button>
                      <button type="button" className="quiet-button" onClick={() => moveSelectedAsset(asset.id, 1)} disabled={busy || index === items.length - 1} aria-label={`下移${asset.name}`}>↓</button>
                      <button type="button" className="quiet-button" onClick={() => removeSelectedAsset(asset.id)} disabled={busy}>移除</button>
                    </div>
                  ))}
                </div>
              ))}
              {!h3PickerAssets(selectedAssets, generationMode).length && <span className="h3-project-media-empty">尚未选择参考素材</span>}
            </div>
          )}
          <div className="asset-video-batch-list" aria-label="视频生产素材列表">
            {h3PickerAssets(selectedAssets, generationMode).map((asset, index) => {
              const isImage = isImageAssetForVideo(asset);
              const promptReady = Boolean(prompts[asset.id]?.trim());
              const checked = selectedIds.has(asset.id);
              const qualification = generationMode.startsWith("REF2VA")
                ? isAudioAssetForH3(asset) ? "可作为音频参考" : isVideoAssetForH3(asset) ? "可作为视频参考" : "可作为图片参考"
                : h3AssetQualification({
                      isImage,
                      promptReady: generationMode.startsWith("FL2VA") ? Boolean(modePrompt) : true,
                      promptTooLong: modePromptTooLong,
                      h3RuntimeReady: contract.ok,
                      comfyConnected,
                      taskEventsReady,
                      durationReady,
                      resolutionReady,
                      resolutionError: resolutionValidation?.errors.width ?? resolutionValidation?.errors.height,
                    });
              return (
                <article key={asset.id} className={`asset-video-batch-card${checked ? " asset-video-batch-card-selected" : ""}`}>
                  <span className="asset-video-select-control">#{index + 1}</span>
                  <div className="asset-video-batch-card-body">
                    <div className="asset-video-batch-card-heading">
                      <strong>{asset.name}</strong>
                      <span className={qualification === "符合条件" ? "asset-prompt-status asset-prompt-status-ready" : "asset-prompt-status"}>
                        {qualification}
                      </span>
                    </div>
                      {isImage && generationMode !== "FL2VA_TEXT_TO_VIDEO" && generationMode.startsWith("FL2VA") && <label className="asset-video-prompt-input">
                        <span>视频提示词</span>
                        <textarea
                        rows={3}
                        maxLength={64 * 1024}
                        value={prompts[asset.id] ?? ""}
                        onChange={(event) => updatePrompt(asset.id, event.target.value)}
                        disabled={busy || !isImage}
                        placeholder="描述运动、方向或变化……"
                      />
                      </label>}
                    {isImage && generationMode !== "FL2VA_TEXT_TO_VIDEO" && generationMode.startsWith("FL2VA") && <div className="asset-video-batch-card-actions">
                      <small>{savedIds.has(asset.id) && !promptReady ? "" : savedIds.has(asset.id) ? "提示词已保存" : "修改后请保存"}</small>
                      <button type="button" className="quiet-button" onClick={() => void savePrompt(asset)} disabled={busy || !isImage || !promptReady || savedIds.has(asset.id)}>
                        保存提示词
                      </button>
                    </div>}
                  </div>
                </article>
              );
            })}
          </div>
          <div className="asset-video-batch-actions">
            <button type="button" onClick={() => void createBatch()} disabled={busy || !canCreate}>
              {busy ? "正在处理..." : `创建视频批次（${batchItems.length}）`}
            </button>
            {createdBatchId && !createdBatchStarted && <button type="button" className="quiet-button" onClick={() => void startBatch()} disabled={busy || productionAdmission.busy}>开始生成</button>}
          </div>
          {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择图片规格中的 16:9 输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
          {batchDraft.error && <p className="error-message" role="alert">视频批次预览失败：{batchDraft.error}</p>}
          {generationMode.startsWith("FL2VA") && missingPromptAssets.length > 0 && !batchPrompt.trim() && <p className="disabled-note">请填写批次 Prompt，或为首帧图片保存视频提示词。</p>}
          {modePromptTooLong && <p className="error-message">视频 Prompt 按 UTF-8 计算不得超过 64 KiB，请缩短后再创建批次。</p>}
          {!modeAssetReady && modeSupported && <p className="disabled-note">请先选择当前模式需要的素材并设置首帧/末帧角色。</p>}
          </div>
          <ProductionQueuePanel
            projectId={projectId}
            batchItems={[]}
            comfyConnected={comfyConnected}
            variant="inline"
            focusBatchId={createdBatchId
              ?? focusProductionBatchId
              ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
            onAdmissionChanged={onAdmissionChanged}
            onFocusedBatchOpened={() => {
              if (focusProductionBatchId) onProductionBatchFocused();
            }}
            onOpenTask={onOpenTask}
            hideCreate
          />
        </div>
      )}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
    </section>
  );
}

interface GenericVideoWorkflowPanelProps {
  projectId: string;
  recipe: RecipeViewModel;
  comfyConnected: boolean;
  taskEventsReady: boolean;
  productionBusy: boolean;
  onOpenTask: (taskId: string) => void;
}

function GenericVideoWorkflowPanel({
  projectId,
  recipe,
  comfyConnected,
  taskEventsReady,
  productionBusy,
  onOpenTask,
}: GenericVideoWorkflowPanelProps) {
  const [values, setValues] = useState<GenerationValues>(() => defaultGenerationValues(recipe));
  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});
  const [missingAssetFields, setMissingAssetFields] = useState<Set<string>>(new Set());
  const [creating, setCreating] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [createdTaskId, setCreatedTaskId] = useState<string>();

  useEffect(() => {
    setValues(defaultGenerationValues(recipe));
    setValidationErrors({});
    setMissingAssetFields(new Set());
    setCreatedTaskId(undefined);
    setNotice(undefined);
  }, [recipe]);

  const errors = validateRecipeValues(recipe, values);
  const canGenerate = comfyConnected
    && taskEventsReady
    && !productionBusy
    && !creating
    && missingAssetFields.size === 0
    && Object.keys(errors).length === 0;

  async function generate() {
    const nextErrors = validateRecipeValues(recipe, values);
    setValidationErrors(nextErrors);
    if (!comfyConnected || !taskEventsReady || productionBusy || missingAssetFields.size > 0 || Object.keys(nextErrors).length > 0) {
      setNotice(!comfyConnected ? "请先连接 ComfyUI。" : !taskEventsReady ? "任务事件通道尚未就绪。" : productionBusy ? "当前有生产队列正在运行。" : "请先补齐通用工作流的必填输入。" );
      return;
    }
    setCreating(true);
    setNotice(undefined);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        values,
      });
      setCreatedTaskId(task.id);
      setNotice("通用视频任务已创建；工作流版本和 Recipe 已冻结。" );
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setCreating(false);
    }
  }

  return (
    <section className="generic-video-workflow-panel" aria-label="通用视频工作流参数">
      <div className="generic-video-workflow-heading">
        <div>
          <span className="section-label">通用视频生成</span>
          <h3>工作流参数</h3>
          <p>使用当前 Recipe 的字段和约束；不会注入 H3 的 FL2VA、REF2VA 或质量参数。</p>
        </div>
        <span className="workflow-selector-origin">自定义</span>
      </div>
      <DynamicFormRenderer
        recipe={recipe}
        values={values}
        validationErrors={validationErrors}
        onChange={(key, value) => setValues((current) => {
          const next = { ...current };
          if (value) next[key] = value;
          else delete next[key];
          return next;
        })}
        onGenerate={() => void generate()}
        projectId={projectId}
        onImageAssetAvailabilityChange={(key, available) => setMissingAssetFields((current) => {
          const next = new Set(current);
          if (available) next.delete(key);
          else next.add(key);
          return next;
        })}
      />
      <div className="generic-video-workflow-actions">
        <button type="button" onClick={() => void generate()} disabled={!canGenerate}>
          {creating ? "正在创建…" : "创建视频任务"}
        </button>
        {createdTaskId && <button type="button" className="quiet-button" onClick={() => onOpenTask(createdTaskId)}>打开任务</button>}
      </div>
      {notice && <p className="studio-notice" role="status">{notice}</p>}
    </section>
  );
}
