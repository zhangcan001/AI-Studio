import { useCallback, useEffect, useRef, useState } from "react";
import {
  commitH3LocalImport,
  pickH3LocalImportDirectory,
  rescanH3LocalImport,
  updateH3ProjectSegmentDraft,
} from "../../../services/tauriClient";
import type { RecipeViewModel } from "../../../types/generation";
import type {
  H3LocalImportInspection,
  H3ProjectSegment,
} from "../../../types/h3LocalImport";
import { toUserMessage } from "../../../i18n/errorMessages";
import {
  MINIMAX_H3_FL2VA_WORKFLOW_ID,
  MINIMAX_H3_WORKFLOW_ID,
  type H3QualityProfile,
} from "../../runtime/productRuntimeScope";
import type { H3CompatibleMode } from "../../runtime/workflowCapabilities";
import {
  type H3GenerationMode,
  type H3RecipeContractResult,
} from "../assetVideoBatch";
import { projectSegmentForm, type ProjectSegmentForm } from "../assetVideoLocalImportModel";

export interface ResolvedLocalImportRecipe {
  mode: H3CompatibleMode;
  recipe?: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">;
  staleManualSelection: boolean;
}

export interface UseAssetVideoLocalImportControllerOptions {
  projectId: string;
  onBusyChange: (busy: boolean) => void;
  onNoticeChange: (notice: string | undefined) => void;
  onAdmissionChanged: () => Promise<void>;
  onCommittedBatch: (result: { batchId: string; autoStarted: boolean }) => void;
}

export interface CommitLocalImportOptions {
  catalog: RecipeViewModel[];
  recipe: RecipeViewModel;
  contract: H3RecipeContractResult;
  generationMode: H3GenerationMode;
  qualityProfile: H3QualityProfile;
  durationSeconds?: number;
  width?: number;
  height?: number;
  resolvedProjectRecipes: readonly ResolvedLocalImportRecipe[];
  canCommit: boolean;
}

export function useAssetVideoLocalImportController({
  projectId,
  onBusyChange,
  onNoticeChange,
  onAdmissionChanged,
  onCommittedBatch,
}: UseAssetVideoLocalImportControllerOptions) {
  const [inspection, setInspection] = useState<H3LocalImportInspection>();
  const [segmentForms, setSegmentForms] = useState<Record<string, ProjectSegmentForm>>({});
  const [batchName, setBatchName] = useState("");
  const [autoStart, setAutoStart] = useState(true);
  const [expandedOrdinal, setExpandedOrdinal] = useState<number>();
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const applyInspection = useCallback((nextInspection: H3LocalImportInspection) => {
    setInspection(nextInspection);
    const segments = nextInspection.projectFolder?.segments ?? [];
    setSegmentForms(Object.fromEntries(segments.map((segment) => [segment.segmentId, projectSegmentForm(segment)])));
  }, []);

  const clearSession = useCallback(() => {
    setInspection(undefined);
    setSegmentForms({});
    setExpandedOrdinal(undefined);
  }, []);

  const updateSegmentForm = useCallback((segmentId: string, patch: Partial<ProjectSegmentForm>) => {
    setSegmentForms((current) => {
      const existing = current[segmentId];
      if (!existing) return current;
      return { ...current, [segmentId]: { ...existing, ...patch } };
    });
  }, []);

  const chooseDirectory = useCallback(async () => {
    onBusyChange(true);
    onNoticeChange(undefined);
    try {
      const nextInspection = await pickH3LocalImportDirectory(projectId, "PROJECT_FOLDER");
      if (!mountedRef.current || !nextInspection) return;
      applyInspection(nextInspection);
      setExpandedOrdinal(undefined);
      onNoticeChange(`已读取「${nextInspection.displayRootName}」，可生成 ${nextInspection.readyCount} 项。`);
    } catch (error: unknown) {
      if (mountedRef.current) onNoticeChange(toUserMessage(error));
    } finally {
      if (mountedRef.current) onBusyChange(false);
    }
  }, [applyInspection, onBusyChange, onNoticeChange, projectId]);

  const rescan = useCallback(async () => {
    const currentInspection = inspection;
    if (!currentInspection) return;
    onBusyChange(true);
    onNoticeChange(undefined);
    try {
      const nextInspection = await rescanH3LocalImport(currentInspection.sessionId, "PROJECT_FOLDER");
      if (!mountedRef.current) return;
      applyInspection(nextInspection);
      setExpandedOrdinal(undefined);
      onNoticeChange(`已重新扫描，当前可生成 ${nextInspection.readyCount} 项。`);
    } catch (error: unknown) {
      if (!mountedRef.current) return;
      clearSession();
      onNoticeChange(toUserMessage(error));
    } finally {
      if (mountedRef.current) onBusyChange(false);
    }
  }, [applyInspection, clearSession, inspection, onBusyChange, onNoticeChange]);

  const saveSegment = useCallback(async (segment: H3ProjectSegment) => {
    const currentInspection = inspection;
    if (!currentInspection?.projectFolder) return;
    const form = segmentForms[segment.segmentId] ?? projectSegmentForm(segment);
    onBusyChange(true);
    onNoticeChange(undefined);
    try {
      const nextInspection = await updateH3ProjectSegmentDraft({
        sessionId: currentInspection.sessionId,
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
      if (!mountedRef.current) return;
      applyInspection(nextInspection);
      onNoticeChange(`已保存第 ${segment.ordinal} 段编辑，提交时将冻结本段参数。`);
    } catch (error: unknown) {
      if (mountedRef.current) onNoticeChange(toUserMessage(error));
    } finally {
      if (mountedRef.current) onBusyChange(false);
    }
  }, [applyInspection, inspection, onBusyChange, onNoticeChange, segmentForms]);

  const resetSegment = useCallback(async (segment: H3ProjectSegment) => {
    const currentInspection = inspection;
    if (!currentInspection?.projectFolder) return;
    onBusyChange(true);
    onNoticeChange(undefined);
    try {
      const nextInspection = await updateH3ProjectSegmentDraft({
        sessionId: currentInspection.sessionId,
        segmentId: segment.segmentId,
        resetAutoDetection: true,
      });
      if (!mountedRef.current) return;
      applyInspection(nextInspection);
      onNoticeChange(`第 ${segment.ordinal} 段已恢复自动识别。`);
    } catch (error: unknown) {
      if (mountedRef.current) onNoticeChange(toUserMessage(error));
    } finally {
      if (mountedRef.current) onBusyChange(false);
    }
  }, [applyInspection, inspection, onBusyChange, onNoticeChange]);

  const commit = useCallback(async ({
    catalog,
    recipe,
    contract,
    generationMode,
    qualityProfile,
    durationSeconds,
    width,
    height,
    resolvedProjectRecipes,
    canCommit,
  }: CommitLocalImportOptions) => {
    if (!contract.ok || !inspection || !canCommit) return;
    const selectedDuration = durationSeconds ?? contract.contract.durationField.default;
    const selectedWidth = width ?? contract.contract.widthField.default;
    const selectedHeight = height ?? contract.contract.heightField.default;
    if (selectedDuration === undefined || selectedWidth === undefined || selectedHeight === undefined) return;
    onBusyChange(true);
    onNoticeChange(undefined);
    try {
      const qualityRecipes = resolvedProjectRecipes
        .flatMap((resolved) => resolved.recipe
          ? [{ mode: resolved.mode, workflowVersionId: resolved.recipe.workflowVersionId, recipeId: resolved.recipe.recipeId }]
          : []);
      const result = await commitH3LocalImport({
        sessionId: inspection.sessionId,
        batchName: batchName.trim() || undefined,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        width: selectedWidth,
        height: selectedHeight,
        durationSeconds: selectedDuration,
        autoStart,
        generationMode,
        fl2vaWorkflowVersionId: catalog.find((item) => item.workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID && item.outputTypes?.includes("video"))?.workflowVersionId,
        fl2vaRecipeId: catalog.find((item) => item.workflowId === MINIMAX_H3_FL2VA_WORKFLOW_ID && item.outputTypes?.includes("video"))?.recipeId,
        ref2vaWorkflowVersionId: catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video"))?.workflowVersionId,
        ref2vaRecipeId: catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video"))?.recipeId,
        qualityProfile,
        qualityRecipes,
      });
      if (!mountedRef.current) return;
      onCommittedBatch({ batchId: result.batchId, autoStarted: result.autoStarted });
      clearSession();
      await onAdmissionChanged();
      onNoticeChange(
        `本地任务已导入，共${result.itemCount}项。${result.autoStarted ? "已开始生成；" : "已创建批次；"}素材已进入资产库。${result.warnings.length ? ` ${result.warnings.join("；")}` : ""}`,
      );
    } catch (error: unknown) {
      if (!mountedRef.current) return;
      clearSession();
      onNoticeChange(`导入未完成，请重新选择项目文件夹。${toUserMessage(error)}`);
    } finally {
      if (mountedRef.current) onBusyChange(false);
    }
  }, [autoStart, batchName, clearSession, inspection, onAdmissionChanged, onBusyChange, onCommittedBatch, onNoticeChange]);

  return {
    inspection,
    segmentForms,
    batchName,
    setBatchName,
    autoStart,
    setAutoStart,
    expandedOrdinal,
    setExpandedOrdinal,
    chooseDirectory,
    rescan,
    updateSegmentForm,
    saveSegment,
    resetSegment,
    clearSession,
    commit,
  };
}
