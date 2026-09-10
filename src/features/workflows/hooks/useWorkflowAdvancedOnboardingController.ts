import { useCallback, useEffect, useState } from "react";
import {
  checkOnboardingCapability,
  commitWorkflowImport,
  discardOnboarding,
  getOnboardingDraft,
  removeOnboardingInputMapping,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
  validateOnboarding,
} from "../../../services/workflowClient";
import { useWorkflowOnboardingStore } from "../../../stores/workflowOnboardingStore";
import type {
  WorkflowInputView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowOnboardingOutputMappingRequest,
} from "../../../types/workflowOnboarding";
import { toUserMessage } from "../../../i18n/errorMessages";
import {
  defaultMapping,
  emptyMapping,
  isExposableWorkflowInput,
  mappingKey,
  optionalNumber,
  optionalText,
  type MappingDraft,
} from "../workflowParameterExposureModel";

export interface OutputDraft {
  outputId: string;
  label: string;
  type: "image" | "video";
  nodeId: string;
  required: boolean;
}

export interface MetadataDraft {
  workflowId: string;
  name: string;
  workflowVersion: string;
  recipeVersion: string;
  category: string;
  mode: string;
}

export function createDefaultOutputDraft(): OutputDraft {
  return {
    outputId: "output_1",
    label: "输出结果",
    type: "image",
    nodeId: "",
    required: true,
  };
}

export interface UseWorkflowAdvancedOnboardingControllerOptions {
  onLoadWorkspace: (mode: "fast" | "refresh") => Promise<void>;
  onCatalogChanged: () => Promise<void>;
  onResetSmartImport?: () => void;
}

export function useWorkflowAdvancedOnboardingController({
  onLoadWorkspace,
  onCatalogChanged,
  onResetSmartImport,
}: UseWorkflowAdvancedOnboardingControllerOptions) {
  const [mappingDrafts, setMappingDrafts] = useState<Record<string, MappingDraft>>({});
  const [outputDraft, setOutputDraft] = useState<OutputDraft>(createDefaultOutputDraft);
  const [metadataDraft, setMetadataDraft] = useState<MetadataDraft>();
  const [published, setPublished] = useState<{ workflowId: string; recipeId: string }>();
  const [showAdvanced, setShowAdvanced] = useState(false);

  const draft = useWorkflowOnboardingStore((state) => state.draft);
  const step = useWorkflowOnboardingStore((state) => state.step);
  const loading = useWorkflowOnboardingStore((state) => state.loading);
  const setDraft = useWorkflowOnboardingStore((state) => state.setDraft);
  const updateDraft = useWorkflowOnboardingStore((state) => state.updateDraft);
  const setStep = useWorkflowOnboardingStore((state) => state.setStep);
  const setLoading = useWorkflowOnboardingStore((state) => state.setLoading);
  const setError = useWorkflowOnboardingStore((state) => state.setError);
  const setNotice = useWorkflowOnboardingStore((state) => state.setNotice);
  const reset = useWorkflowOnboardingStore((state) => state.reset);

  useEffect(() => {
    if (!draft) {
      setMetadataDraft(undefined);
      setMappingDrafts({});
      setPublished(undefined);
      return;
    }
    setMetadataDraft({
      workflowId: draft.manifest.workflowId,
      name: draft.manifest.name,
      workflowVersion: draft.manifest.workflowVersion,
      recipeVersion: draft.manifest.recipeVersion,
      category: draft.manifest.category,
      mode: draft.manifest.mode,
    });
    const firstOutputNode = draft.nodes.find((node) => node.isOutputNode) ?? draft.nodes[0];
    setOutputDraft((current) => ({
      ...current,
      nodeId: firstOutputNode?.nodeId ?? "",
    }));
    setPublished(undefined);
    setMappingDrafts({});
  }, [draft?.draftId]);

  const resetSession = useCallback(() => {
    setMappingDrafts({});
    setOutputDraft(createDefaultOutputDraft());
    setMetadataDraft(undefined);
    setPublished(undefined);
    setShowAdvanced(false);
  }, []);

  const openAdvanced = useCallback((nextDraft?: WorkflowOnboardingDraftView) => {
    if (nextDraft) setDraft(nextDraft);
    setShowAdvanced(true);
  }, [setDraft]);

  const hideAdvanced = useCallback(() => {
    setShowAdvanced(false);
  }, []);

  const runDraftAction = useCallback(async (action: () => Promise<void>) => {
    setLoading(true);
    setError(undefined);
    try {
      await action();
    } catch (actionError: unknown) {
      setError(toUserMessage(actionError));
    } finally {
      setLoading(false);
    }
  }, [setError, setLoading]);

  const checkCapability = useCallback(async () => {
    if (!draft) return;
    await runDraftAction(async () => {
      await checkOnboardingCapability(draft.draftId);
      updateDraft(await getOnboardingDraft(draft.draftId));
      setStep("compatibility");
    });
  }, [draft, runDraftAction, setStep, updateDraft]);

  const validateDraft = useCallback(async () => {
    if (!draft) return;
    await runDraftAction(async () => {
      const validation = await validateOnboarding(draft.draftId);
      updateDraft({ ...draft, validation });
      setStep("validate");
    });
  }, [draft, runDraftAction, setStep, updateDraft]);

  const publishDraft = useCallback(async () => {
    if (!draft || !draft.validation.readyToPublish) return;
    await runDraftAction(async () => {
      const result = await commitWorkflowImport({
        draftId: draft.draftId,
        action: "NEW_WORKFLOW",
        setCurrent: false,
      });
      setPublished({ workflowId: result.workflowId, recipeId: result.recipeId });
      setNotice(`已发布 ${result.packageName}，运行目录已刷新。`);
      await onLoadWorkspace("refresh");
      await onCatalogChanged();
      setStep("publish");
    });
  }, [draft, onCatalogChanged, onLoadWorkspace, runDraftAction, setNotice, setStep]);

  const discardDraft = useCallback(async () => {
    if (!draft) return;
    setLoading(true);
    setError(undefined);
    let discardError: string | undefined;
    try {
      await discardOnboarding(draft.draftId);
    } catch (actionError: unknown) {
      discardError = toUserMessage(actionError);
    } finally {
      reset();
      resetSession();
      onResetSmartImport?.();
      setLoading(false);
    }
    if (discardError) {
      setError(discardError);
    } else {
      setNotice("草稿已丢弃。");
    }
  }, [draft, onResetSmartImport, reset, resetSession, setError, setLoading, setNotice]);

  const saveMetadata = useCallback(async () => {
    if (!draft || !metadataDraft) return;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingMetadata(draft.draftId, metadataDraft);
      updateDraft(nextDraft);
      setNotice("基本信息已保存，请重新校验后再发布。");
    });
  }, [draft, metadataDraft, runDraftAction, setNotice, updateDraft]);

  const bindInput = useCallback(async (nodeId: string, input: WorkflowInputView) => {
    if (!draft || (!input.bindable && (!input.isLinked || !isExposableWorkflowInput(input)))) return;
    const mapping = mappingDrafts[mappingKey(nodeId, input.name)] ?? defaultMapping(nodeId, input);
    const request: WorkflowOnboardingInputMappingRequest = {
      semanticKey: mapping.semanticKey,
      fieldType: mapping.fieldType,
      label: mapping.label,
      required: mapping.required,
      defaultValue: optionalText(mapping.defaultValue),
      minValue: optionalText(mapping.minValue),
      maxValue: optionalText(mapping.maxValue),
      step: optionalText(mapping.step),
      minItems: optionalNumber(mapping.minItems),
      maxItems: optionalNumber(mapping.maxItems),
      itemIndex: optionalNumber(mapping.itemIndex),
      targetNode: nodeId,
      targetInput: input.name,
    };
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingInputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${mapping.label} 已绑定到 ${nodeId}.${input.name}。`);
    });
  }, [draft, mappingDrafts, runDraftAction, setNotice, updateDraft]);

  const removeInput = useCallback(async (mapping: WorkflowOnboardingDraftView["inputMappings"][number]) => {
    if (!draft) return;
    await runDraftAction(async () => {
      const nextDraft = await removeOnboardingInputMapping(draft.draftId, {
        semanticKey: mapping.semanticKey,
        itemIndex: mapping.itemIndex,
      });
      updateDraft(nextDraft);
    });
  }, [draft, runDraftAction, updateDraft]);

  const addOutput = useCallback(async () => {
    if (!draft || !outputDraft.nodeId) return;
    const request: WorkflowOnboardingOutputMappingRequest = outputDraft;
    await runDraftAction(async () => {
      const nextDraft = await setOnboardingOutputMapping(draft.draftId, request);
      updateDraft(nextDraft);
      setNotice(`${outputDraft.label} 已设置为${outputDraft.type === "video" ? "视频" : "图片"}输出。`);
    });
  }, [draft, outputDraft, runDraftAction, setNotice, updateDraft]);

  const patchMapping = useCallback((key: string, patch: Partial<MappingDraft>) => {
    setMappingDrafts((current) => ({
      ...current,
      [key]: { ...(current[key] ?? emptyMapping()), ...patch },
    }));
  }, []);

  return {
    draft,
    step,
    loading,
    showAdvanced,
    mappingDrafts,
    outputDraft,
    metadataDraft,
    published,
    setPublished,
    setOutputDraft,
    setMetadataDraft,
    patchMapping,
    openAdvanced,
    hideAdvanced,
    resetSession,
    checkCapability,
    validateDraft,
    publishDraft,
    discardDraft,
    saveMetadata,
    bindInput,
    removeInput,
    addOutput,
  };
}
