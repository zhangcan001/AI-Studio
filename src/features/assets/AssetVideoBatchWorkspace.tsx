import { useEffect, useMemo, useState } from "react";
import {
  commitH3LocalImport,
  createProductionQueue,
  listAssetVideoPrompts,
  pickH3LocalImportDirectory,
  rescanH3LocalImport,
  setAssetVideoPrompt,
  startProductionQueue,
  updateH3ProjectSegmentDraft,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import type {
  H3LocalImportInspection,
  H3LocalImportMode,
  H3ProjectSegment,
  H3ProjectGenerationMode,
  H3ProjectFolderInspection,
} from "../../types/h3LocalImport";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { toUserMessage } from "../../i18n/errorMessages";
import { MINIMAX_H3_FL2VA_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { ResolutionControl } from "../runtime/ResolutionControl";
import { MINIMAX_H3_RESOLUTION_PRESETS, resolutionPresetsForRecipe } from "../runtime/resolutionPresets";
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
import { formatPromptBytes, localImportCanCommit, localImportModeLabel, localImportStatusLabel } from "./h3LocalImport";

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
}

function localImportModeForGeneration(mode: H3GenerationMode): H3LocalImportMode {
  switch (mode) {
    case "FL2VA_TEXT_TO_VIDEO": return "TEXT";
    case "FL2VA_FIRST_LAST": return "FIRST_LAST";
    case "REF2VA_AUDIO":
    case "REF2VA_IMAGE_AUDIO":
    case "REF2VA_VIDEO_IMAGE": return "OMNI_MANIFEST";
    default: return "PAIRING";
  }
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
                  <label><span>宽度</span><input type="number" min={32} max={2048} step={32} value={form.width} onChange={(event) => onChange(segment.segmentId, { width: Number(event.target.value) })} disabled={busy} /></label>
                  <label><span>高度</span><input type="number" min={32} max={2048} step={32} value={form.height} onChange={(event) => onChange(segment.segmentId, { height: Number(event.target.value) })} disabled={busy} /></label>
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
                  <span>模式来源：{segment.modeSource} · 分辨率：{segment.resolutionSource} · 时长：{segment.durationSource}</span>
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
}: Props) {
  const [sourceMode, setSourceMode] = useState<"ASSET_LIBRARY" | "LOCAL_FOLDER">("ASSET_LIBRARY");
  const [localMode, setLocalMode] = useState<H3LocalImportMode>("PROJECT_FOLDER");
  const [localInspection, setLocalInspection] = useState<H3LocalImportInspection>();
  const [projectSegmentForms, setProjectSegmentForms] = useState<Record<string, ProjectSegmentForm>>({});
  const [localBatchName, setLocalBatchName] = useState("");
  const [localAutoStart, setLocalAutoStart] = useState(true);
  const [expandedLocalOrdinal, setExpandedLocalOrdinal] = useState<number>();
  const [generationMode, setGenerationMode] = useState<H3GenerationMode>("FL2VA_TEXT_TO_VIDEO");
  const [batchPrompt, setBatchPrompt] = useState("");
  const [firstFrameAssetId, setFirstFrameAssetId] = useState<string>();
  const [lastFrameAssetId, setLastFrameAssetId] = useState<string>();
  const recipe = useMemo(
    () => {
      const workflowId = generationMode.startsWith("FL2VA")
        ? MINIMAX_H3_FL2VA_WORKFLOW_ID
        : MINIMAX_H3_WORKFLOW_ID;
      return catalog.find((item) => item.workflowId === workflowId && item.outputTypes?.includes("video"))
        ?? (workflowId === MINIMAX_H3_WORKFLOW_ID
          ? catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video"))
          : undefined);
    },
    [catalog, generationMode],
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

  useEffect(() => {
    let active = true;
    setPrompts({});
    setSavedIds(new Set());
    setSelectedIds(new Set(initialAssets.map((asset) => asset.id)));
    if (!initialAssets.length) return () => { active = false; };
    void listAssetVideoPrompts(projectId, initialAssets.map((asset) => asset.id))
      .then((records) => {
        if (!active) return;
        setPrompts(Object.fromEntries(records.map((record) => [record.assetId, record.promptText])));
        setSavedIds(new Set(records.map((record) => record.assetId)));
      })
      .catch((error: unknown) => {
        if (active) setNotice(toUserMessage(error));
      });
    return () => { active = false; };
  }, [initialAssets, projectId]);

  useEffect(() => {
    setDurationSeconds(contract.ok ? contract.contract.durationField.default : undefined);
    setWidth(contract.ok ? contract.contract.widthField.default : undefined);
    setHeight(contract.ok ? contract.contract.heightField.default : undefined);
  }, [contract]);

  useEffect(() => {
    const first = initialAssets.find((asset) => isImageAssetForVideo(asset));
    setFirstFrameAssetId((current) => current && initialAssets.some((asset) => asset.id === current) ? current : first?.id);
    setLastFrameAssetId((current) => current && initialAssets.some((asset) => asset.id === current) ? current : undefined);
  }, [initialAssets]);

  useEffect(() => {
    setLocalMode((current) => current === "PROJECT_FOLDER" ? current : localImportModeForGeneration(generationMode));
    setLocalInspection(undefined);
    setProjectSegmentForms({});
    setExpandedLocalOrdinal(undefined);
  }, [generationMode]);

  const selectedAssets = initialAssets.filter((asset) => selectedIds.has(asset.id));
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
  const resolutionReady = Boolean(resolutionValidation?.ok);
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
  const localCanCreate = localImportCanCommit(
    localInspection,
    localInspection?.mode === "PROJECT_FOLDER"
      ? Boolean(comfyConnected && taskEventsReady && catalog.some((item) => item.workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID && item.outputTypes?.includes("video")) && catalog.some((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video")))
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

  function toggleAsset(assetId: string) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(assetId)) next.delete(assetId);
      else if (next.size < 100) next.add(assetId);
      return next;
    });
  }

  function isSelectableForMode(asset: AssetView): boolean {
    switch (generationMode) {
      case "FL2VA_TEXT_TO_VIDEO": return false;
      case "FL2VA_IMAGE_TO_VIDEO":
      case "FL2VA_FIRST_LAST":
      case "REF2VA_IMAGE":
        return isImageAssetForVideo(asset);
      case "REF2VA_IMAGE_AUDIO":
        return isImageAssetForVideo(asset) || isAudioAssetForH3(asset);
      case "REF2VA_VIDEO_IMAGE":
        return isImageAssetForVideo(asset) || isVideoAssetForH3(asset);
      case "REF2VA_AUDIO": return isAudioAssetForH3(asset);
    }
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

  async function chooseLocalDirectory() {
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await pickH3LocalImportDirectory(projectId, localMode);
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

  async function changeLocalMode(nextMode: H3LocalImportMode) {
    setLocalMode(nextMode);
    if (!localInspection) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await rescanH3LocalImport(localInspection.sessionId, nextMode);
      applyLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setProjectSegmentForms({});
      setExpandedLocalOrdinal(undefined);
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function rescanLocalDirectory() {
    if (!localInspection) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await rescanH3LocalImport(localInspection.sessionId, localMode);
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
      setNotice(`导入未完成，请重新选择任务目录。${toUserMessage(error)}`);
    } finally {
      setBusy(false);
    }
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
            const available = catalog.some((item) => item.workflowId === (family === "FL2VA" ? MINIMAX_H3_FL2VA_WORKFLOW_ID : MINIMAX_H3_WORKFLOW_ID) && item.outputTypes?.includes("video"));
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
            const optionRecipe = catalog.find((item) => item.workflowId === (option.family === "FL2VA" ? MINIMAX_H3_FL2VA_WORKFLOW_ID : MINIMAX_H3_WORKFLOW_ID) && item.outputTypes?.includes("video"));
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

      <section className="h3-safety-card" aria-label="H3 安全配置">
        <div>
          <strong>MiniMax H3</strong>
          <p>模型产品能力：最高 15 秒 · 最高 2K</p>
          <p>当前 Runtime：4 步 · 单任务串行</p>
          <small>本机历史已验证：0.1 MP · 5 秒 · RTX 5060 Ti 16GB</small>
        </div>
        {contract.ok && (
          <ResolutionControl
            widthField={contract.contract.widthField}
            heightField={contract.contract.heightField}
            width={selectedWidth}
            height={selectedHeight}
            presets={resolutionPresets}
            disabled={busy}
            onChange={(next) => {
              setWidth(next.width);
              setHeight(next.height);
            }}
          />
        )}
        <div className="h3-duration-control">
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
        </div>
        <small>{recipe ? `运行时已锁定：${recipe.workflowId}` : "运行时未就绪"}</small>
      </section>
      {exceedsHistoricalProfile && (
        <p className="h3-profile-warning" role="status">
          当前配置超出本机已验证范围。模型/产品允许该配置，但 RTX 5060 Ti 16GB 的显存占用尚未验证，生成时可能出现显存不足。
        </p>
      )}

      {sourceMode === "LOCAL_FOLDER" ? (
        <div className="h3-local-import-layout">
          <section className="h3-local-import-panel" aria-label="MiniMax H3 本地批量导入">
            <div className="section-heading">
              <div>
                <span className="section-label">本地批量</span>
                <h3>MiniMax H3 本地导入</h3>
                <p className="section-description">选择任务目录，检查图片与 Prompt 后导入资产库并创建普通视频生产队列。</p>
              </div>
              <button type="button" onClick={() => void chooseLocalDirectory()} disabled={busy}>
                {busy ? "处理中…" : "选择任务目录"}
              </button>
            </div>

            <div className="h3-local-import-controls">
              <label>
                <span>导入方式</span>
                <select value={localMode} onChange={(event) => void changeLocalMode(event.target.value as H3LocalImportMode)} disabled={busy}>
                  <option value="PAIRING">{localImportModeLabel("PAIRING")}</option>
                  <option value="MANIFEST">{localImportModeLabel("MANIFEST")}</option>
                  <option value="TEXT">{localImportModeLabel("TEXT")}</option>
                  <option value="FIRST_LAST">{localImportModeLabel("FIRST_LAST")}</option>
                  <option value="OMNI_MANIFEST">{localImportModeLabel("OMNI_MANIFEST")}</option>
                  <option value="PROJECT_FOLDER">{localImportModeLabel("PROJECT_FOLDER")}</option>
                </select>
              </label>
              {localInspection && (
                <button type="button" className="quiet-button" onClick={() => void rescanLocalDirectory()} disabled={busy}>
                  重新扫描
                </button>
              )}
            </div>

            {!localInspection ? (
              <div className="h3-local-import-empty">
                <strong>尚未选择任务目录</strong>
                <span>目录只在本机短时使用；页面不会显示或保存绝对路径。</span>
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
                {localInspection.detectedManifest && localMode === "PAIRING" && (
                  <p className="h3-local-import-warning" role="status">检测到 h3-batch.json；当前使用自动同名配对，可切换为 JSON 批量清单。</p>
                )}
                {localInspection.warnings.map((warning) => <p key={warning} className="h3-local-import-warning" role="status">{warning}</p>)}
                {localInspection.errors.length > 0 && (
                  <div className="h3-local-import-errors" role="alert">
                    <strong>导入前需要修复：</strong>
                    <ul>{localInspection.errors.slice(0, 12).map((error) => <li key={error}>{error}</li>)}</ul>
                    {localInspection.errors.length > 12 && <small>还有 {localInspection.errors.length - 12} 项异常，请修复后重新扫描。</small>}
                  </div>
                )}
                {localInspection.mode === "PROJECT_FOLDER" && localInspection.projectFolder ? (
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
                {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择符合 Recipe 约束的输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
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
      ) : generationMode !== "FL2VA_TEXT_TO_VIDEO" && !initialAssets.length ? (
        <div className="asset-video-empty-state">
          <strong>还没有选择素材</strong>
          <p>回到资产库，勾选当前模式需要的图片、视频或音频素材。</p>
          <button type="button" onClick={onBackToAssets}>去资产库选择</button>
        </div>
      ) : (
        <div className="batch-workspace-grid">
          <div className="batch-editor-column">
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
          <div className="asset-video-batch-list" aria-label="视频生产素材列表">
            {initialAssets.map((asset, index) => {
              const isImage = isImageAssetForVideo(asset);
              const isSelectable = isSelectableForMode(asset);
              const promptReady = Boolean(prompts[asset.id]?.trim());
              const checked = selectedIds.has(asset.id);
              const qualification = !isSelectable
                ? "该模式不使用此素材"
                : (generationMode === "REF2VA_AUDIO" || (generationMode === "REF2VA_IMAGE_AUDIO" && isAudioAssetForH3(asset)))
                  ? "可作为音频参考"
                  : generationMode === "REF2VA_VIDEO_IMAGE" && isVideoAssetForH3(asset)
                    ? "可作为视频参考"
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
                  <label className="asset-video-select-control">
                    <input type="checkbox" checked={checked} onChange={() => toggleAsset(asset.id)} disabled={busy || !isSelectable} />
                    <span>#{index + 1}</span>
                  </label>
                  <div className="asset-video-batch-card-body">
                    <div className="asset-video-batch-card-heading">
                      <strong>{asset.name}</strong>
                      <span className={qualification === "符合条件" ? "asset-prompt-status asset-prompt-status-ready" : "asset-prompt-status"}>
                        {qualification}
                      </span>
                    </div>
                      {isImage && generationMode.startsWith("FL2VA") && <label className="asset-video-prompt-input">
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
                    {isImage && generationMode.startsWith("FL2VA") && <div className="asset-video-batch-card-actions">
                      <small>{savedIds.has(asset.id) && !promptReady ? "" : savedIds.has(asset.id) ? "提示词已保存" : "修改后请保存"}</small>
                      <button type="button" className="quiet-button" onClick={() => void savePrompt(asset)} disabled={busy || !isImage || !promptReady || savedIds.has(asset.id)}>
                        保存提示词
                      </button>
                    </div>}
                    {generationMode === "FL2VA_FIRST_LAST" && isImage && <div className="asset-video-frame-role-actions">
                      <button type="button" className={firstFrameAssetId === asset.id ? "quiet-button asset-role-active" : "quiet-button"} onClick={() => setFirstFrameAssetId(asset.id)} disabled={busy}>设为首帧</button>
                      <button type="button" className={lastFrameAssetId === asset.id ? "quiet-button asset-role-active" : "quiet-button"} onClick={() => setLastFrameAssetId(asset.id)} disabled={busy}>设为末帧</button>
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
          {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择符合 Recipe 约束的输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
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
