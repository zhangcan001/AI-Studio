import { useCallback, useEffect, useRef, useState } from "react";
import {
  createPreset,
  deletePreset,
  getPreferredPreset,
  listPresets,
  setPreferredPreset,
  updatePreset,
} from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import { useStudioStore } from "../../../stores/studioStore";
import type { RecipeViewModel } from "../../../types/generation";
import type { PresetView } from "../../../types/preset";

export interface UseGenerationPresetControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
  onPresetApplied?: () => void;
  onNotice?: (message: string) => void;
}

export function useGenerationPresetController({
  projectId,
  selectedWorkflow,
  onPresetApplied,
  onNotice,
}: UseGenerationPresetControllerOptions) {
  const values = useStudioStore((state) => state.values);
  const [presets, setPresets] = useState<PresetView[]>([]);
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [preferredPresetId, setPreferredPresetId] = useState<string | null>(null);
  const [presetName, setPresetName] = useState("");
  const [presetLoading, setPresetLoading] = useState(false);
  const [presetError, setPresetError] = useState<string>();
  const [presetEditorOpen, setPresetEditorOpen] = useState(false);
  const mountedRef = useRef(true);
  const lifecycleRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    const lifecycle = ++lifecycleRef.current;
    let active = true;
    setPresets([]);
    setSelectedPresetId("");
    setPreferredPresetId(null);
    setPresetName("");
    setPresetError(undefined);
    setPresetEditorOpen(false);

    if (!selectedWorkflow) {
      setPresetLoading(false);
      return () => {
        active = false;
        if (lifecycleRef.current === lifecycle) lifecycleRef.current += 1;
      };
    }

    setPresetLoading(true);
    void Promise.all([
      listPresets(projectId, selectedWorkflow.workflowVersionId, selectedWorkflow.recipeId),
      getPreferredPreset(projectId, selectedWorkflow.workflowVersionId, selectedWorkflow.recipeId),
    ])
      .then(([nextPresets, nextPreferredId]) => {
        if (!active || !mountedRef.current || lifecycleRef.current !== lifecycle) return;
        setPresets(nextPresets);
        setPreferredPresetId(nextPreferredId);
        const preferred = nextPreferredId
          ? nextPresets.find((preset) => preset.id === nextPreferredId)
          : undefined;
        if (preferred && !useStudioStore.getState().draftDirty) {
          useStudioStore.getState().loadDraft(selectedWorkflow, preferred.values);
          setSelectedPresetId(preferred.id);
          setPresetName(preferred.name);
        }
      })
      .catch((loadError: unknown) => {
        if (active && mountedRef.current && lifecycleRef.current === lifecycle) {
          setPresetError(toUserMessage(loadError));
        }
      })
      .finally(() => {
        if (active && mountedRef.current && lifecycleRef.current === lifecycle) {
          setPresetLoading(false);
        }
      });

    return () => {
      active = false;
      if (lifecycleRef.current === lifecycle) lifecycleRef.current += 1;
    };
  }, [projectId, selectedWorkflow]);

  const applyPreset = useCallback((preset: PresetView) => {
    if (!selectedWorkflow) return;
    useStudioStore.getState().loadDraft(selectedWorkflow, preset.values);
    setSelectedPresetId(preset.id);
    setPresetName(preset.name);
    setPresetEditorOpen(false);
    setPresetError(undefined);
    onPresetApplied?.();
  }, [onPresetApplied, selectedWorkflow]);

  const clearSelection = useCallback(() => {
    setSelectedPresetId("");
    setPresetName("");
    setPresetEditorOpen(false);
  }, []);

  const openEditor = useCallback(() => {
    setPresetName("");
    setPresetError(undefined);
    setPresetEditorOpen(true);
  }, []);

  const savePreset = useCallback(async () => {
    if (!selectedWorkflow) return;
    if (!presetName.trim()) {
      setPresetError("请输入预设名称后再保存。");
      return;
    }
    const lifecycle = lifecycleRef.current;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const preset = await createPreset({
        projectId,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        name: presetName,
        values,
      });
      if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
      setPresets((current) => [preset, ...current.filter((item) => item.id !== preset.id)]);
      applyPreset(preset);
    } catch (saveError: unknown) {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetError(toUserMessage(saveError));
    } finally {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetLoading(false);
    }
  }, [applyPreset, presetName, projectId, selectedWorkflow, values]);

  const savePresetChanges = useCallback(async () => {
    if (!selectedPresetId) {
      await savePreset();
      return;
    }
    if (!presetName.trim()) {
      setPresetError("请输入预设名称后再保存。");
      return;
    }
    const lifecycle = lifecycleRef.current;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const preset = await updatePreset({ projectId, presetId: selectedPresetId, name: presetName, values });
      if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
      setPresets((current) => current.map((item) => (item.id === preset.id ? preset : item)));
      applyPreset(preset);
    } catch (updateError: unknown) {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetError(toUserMessage(updateError));
    } finally {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetLoading(false);
    }
  }, [applyPreset, presetName, projectId, savePreset, selectedPresetId, values]);

  const removePreset = useCallback(async () => {
    if (!selectedPresetId) return;
    if (!window.confirm("确定删除这个预设吗？")) return;
    const presetId = selectedPresetId;
    const lifecycle = lifecycleRef.current;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      if (preferredPresetId === presetId) {
        await setPreferredPreset({
          projectId,
          workflowVersionId: selectedWorkflow?.workflowVersionId ?? "",
          recipeId: selectedWorkflow?.recipeId ?? "",
        });
        if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
        setPreferredPresetId(null);
      }
      await deletePreset(projectId, presetId);
      if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
      setPresets((current) => current.filter((preset) => preset.id !== presetId));
      clearSelection();
    } catch (deleteError: unknown) {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetError(toUserMessage(deleteError));
    } finally {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetLoading(false);
    }
  }, [clearSelection, preferredPresetId, projectId, selectedPresetId, selectedWorkflow]);

  const togglePreferredPreset = useCallback(async () => {
    if (!selectedWorkflow || !selectedPresetId) return;
    const lifecycle = lifecycleRef.current;
    setPresetLoading(true);
    setPresetError(undefined);
    try {
      const nextPreferredId = preferredPresetId === selectedPresetId ? undefined : selectedPresetId;
      await setPreferredPreset({
        projectId,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        presetId: nextPreferredId,
      });
      if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
      setPreferredPresetId(nextPreferredId ?? null);
      onNotice?.(nextPreferredId ? "已设为当前工作流默认预设。" : "已取消当前工作流默认预设。");
    } catch (preferredError: unknown) {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetError(toUserMessage(preferredError));
    } finally {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setPresetLoading(false);
    }
  }, [onNotice, preferredPresetId, projectId, selectedPresetId, selectedWorkflow]);

  return {
    presets,
    selectedPresetId,
    preferredPresetId,
    presetName,
    presetLoading,
    presetError,
    presetEditorOpen,
    setPresetName,
    setPresetEditorOpen,
    clearSelection,
    openEditor,
    applyPreset,
    savePreset,
    savePresetChanges,
    removePreset,
    togglePreferredPreset,
  };
}
