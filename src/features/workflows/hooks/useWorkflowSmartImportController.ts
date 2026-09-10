import { useCallback, useRef, useState } from "react";
import type { MutableRefObject } from "react";
import {
  analyzeWorkflowImport,
  commitWorkflowImport,
  discardOnboarding,
  getOnboardingDraft,
  rerecognizeWorkflow,
  setOnboardingInputMapping,
  setOnboardingMetadata,
  setOnboardingOutputMapping,
} from "../../../services/workflowClient";
import { useWorkflowOnboardingStore } from "../../../stores/workflowOnboardingStore";
import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowFieldType,
  WorkflowImportCommitAction,
  WorkflowInputView,
  WorkflowOnboardingDraftView,
  WorkflowOnboardingInputMappingRequest,
  WorkflowProductionWorkspaceView,
} from "../../../types/workflowOnboarding";
import { toUserMessage } from "../../../i18n/errorMessages";
import { workflowImportFormat } from "../WorkflowSmartImport";
import { workflowImportErrorView, nextWorkflowVersion } from "../workflowSmartImportModel";
import type { WorkflowWorkspaceItem } from "../workflowWorkspaceAdapters";

type PublishedWorkflow = { workflowId: string; recipeId: string };

export interface UseWorkflowSmartImportControllerOptions {
  workspaceItems: readonly WorkflowWorkspaceItem[];
  importBusyRef?: MutableRefObject<boolean>;
  onLoadWorkspace: (mode: "fast" | "refresh") => Promise<void>;
  onCatalogChanged: () => Promise<void>;
  onDiscardReplacedDraft: (previousDraftId: string | undefined, nextDraftId?: string) => Promise<void>;
  onResetImportView: () => void;
  onCloseAdvanced: () => void;
  onAdvancedRequested: (draft?: WorkflowOnboardingDraftView) => void;
  onPublished: (published?: PublishedWorkflow) => void;
  onOpenStudio: (workflowId: string, recipeId: string) => Promise<void>;
}

export function useWorkflowSmartImportController({
  workspaceItems,
  importBusyRef: externalBusyRef,
  onLoadWorkspace,
  onCatalogChanged,
  onDiscardReplacedDraft,
  onResetImportView,
  onCloseAdvanced,
  onAdvancedRequested,
  onPublished,
  onOpenStudio,
}: UseWorkflowSmartImportControllerOptions) {
  const [plan, setPlan] = useState<WorkflowAutoOnboardingPlanView>();
  const [importError, setImportError] = useState<ReturnType<typeof workflowImportErrorView>>();
  const internalBusyRef = useRef(false);
  const busyRef = externalBusyRef ?? internalBusyRef;

  const draft = useWorkflowOnboardingStore((state) => state.draft);
  const loading = useWorkflowOnboardingStore((state) => state.loading);
  const setDraft = useWorkflowOnboardingStore((state) => state.setDraft);
  const setLoading = useWorkflowOnboardingStore((state) => state.setLoading);
  const setError = useWorkflowOnboardingStore((state) => state.setError);
  const setNotice = useWorkflowOnboardingStore((state) => state.setNotice);
  const reset = useWorkflowOnboardingStore((state) => state.reset);

  const resetSession = useCallback(() => {
    setPlan(undefined);
    setImportError(undefined);
    onResetImportView();
  }, [onResetImportView]);

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

  const smartImport = useCallback(async (existingWorkflowId?: string) => {
    if (loading || busyRef.current) return;
    const previousDraftId = draft?.draftId;
    busyRef.current = true;
    setLoading(true);
    try {
      const analyzed = await analyzeWorkflowImport(existingWorkflowId);
      if (analyzed) {
        await onDiscardReplacedDraft(previousDraftId, analyzed.draftId);
        reset();
        setLoading(true);
        resetSession();
        const analyzedPlan = analyzed.published || analyzed.state === "AUTO_PUBLISHED"
          ? {
              ...analyzed,
              state: "NEEDS_REVIEW" as const,
              commitRequired: true,
              published: undefined,
              message: "识别已完成，请明确点击添加工作流后写入。",
            }
          : analyzed;
        setPlan(analyzedPlan);
        const detectedFormat = workflowImportFormat(analyzedPlan);
        if (analyzedPlan.draftId && (!detectedFormat || detectedFormat === "API")) {
          setDraft(await getOnboardingDraft(analyzedPlan.draftId));
        } else if (analyzedPlan.draftId && detectedFormat && detectedFormat !== "API") {
          await discardOnboarding(analyzedPlan.draftId).catch(() => undefined);
        }
        setNotice(undefined);
      } else {
        await onDiscardReplacedDraft(previousDraftId);
        reset();
        setLoading(true);
        resetSession();
      }
    } catch (importErrorValue: unknown) {
      await onDiscardReplacedDraft(previousDraftId);
      reset();
      resetSession();
      setError(undefined);
      setImportError(workflowImportErrorView(importErrorValue));
    } finally {
      setLoading(false);
      busyRef.current = false;
    }
  }, [busyRef, draft?.draftId, loading, onDiscardReplacedDraft, reset, resetSession, setDraft, setError, setLoading, setNotice]);

  const resume = useCallback(async () => {
    if (!plan) return;
    await runDraftAction(async () => {
      const nextPlan = await analyzeWorkflowImport(plan.existingWorkflowId);
      if (!nextPlan) return;
      setPlan(nextPlan);
      if (nextPlan.draftId) setDraft(await getOnboardingDraft(nextPlan.draftId));
      setNotice(nextPlan.message || "识别已刷新，请确认后再添加。");
    });
  }, [plan, runDraftAction, setDraft, setNotice]);

  const regenerateRecipe = useCallback(async () => {
    if (!plan?.existingWorkflowId || !plan.existingWorkflowVersion) return;
    await runDraftAction(async () => {
      const nextPlan = plan.draftId
        ? plan
        : await rerecognizeWorkflow(plan.existingWorkflowId!);
      if (nextPlan.draftId !== plan.draftId) setPlan(nextPlan);
      const result = await commitWorkflowImport({
        draftId: nextPlan.draftId,
        action: "NEW_RECIPE",
        workflowId: nextPlan.existingWorkflowId,
        setCurrent: false,
      });
      const workflowId = result.workflowId ?? nextPlan.existingWorkflowId!;
      const recipeId = result.recipeId ?? nextPlan.metadata.recipeId;
      onPublished({ workflowId, recipeId });
      setPlan({
        ...nextPlan,
        state: "AUTO_PUBLISHED",
        commitRequired: false,
        published: result,
      });
      setNotice(`已为现有工作流新增 Recipe ${result.recipeVersion ?? ""}。原工作流版本和旧 Recipe 保持不变。`);
      await onLoadWorkspace("refresh");
      await onCatalogChanged();
    });
  }, [onCatalogChanged, onLoadWorkspace, onPublished, plan, runDraftAction, setNotice]);

  const resolveIssue = useCallback(async (issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) => {
    if (!plan || !draft) return;
    await runDraftAction(async () => {
      if (issue.code === "AMBIGUOUS_OUTPUT" && candidate.nodeId && candidate.outputType) {
        await setOnboardingOutputMapping(plan.draftId, {
          outputId: candidate.outputId ?? "output_1",
          label: candidate.label,
          type: candidate.outputType as "image" | "video",
          nodeId: candidate.nodeId,
          required: true,
        });
      } else if (candidate.nodeId && candidate.inputName) {
        const node = draft.nodes.find((item) => item.nodeId === candidate.nodeId);
        const input = node?.inputs.find((item) => item.name === candidate.inputName);
        if (!input) return;
        const base = defaultInputMapping(candidate.nodeId, input);
        const fieldType = candidate.fieldType && fieldTypes.includes(candidate.fieldType as WorkflowFieldType)
          ? candidate.fieldType as WorkflowFieldType
          : base.fieldType;
        const request: WorkflowOnboardingInputMappingRequest = {
          semanticKey: issue.field ?? base.semanticKey,
          fieldType,
          label: base.label,
          required: base.required,
          defaultValue: optionalText(base.defaultValue),
          minValue: optionalText(base.minValue),
          maxValue: optionalText(base.maxValue),
          step: input.numericStep,
          minItems: optionalNumber(base.minItems),
          maxItems: optionalNumber(base.maxItems),
          itemIndex: optionalNumber(base.itemIndex),
          targetNode: candidate.nodeId,
          targetInput: candidate.inputName,
        };
        await setOnboardingInputMapping(plan.draftId, request);
      }
      setNotice("已记录这项选择，请重新分析工作流后再添加。");
    });
  }, [draft, plan, runDraftAction, setNotice]);

  const commit = useCallback(async (action: WorkflowImportCommitAction = "NEW_WORKFLOW") => {
    if (!plan?.draftId) return;
    await runDraftAction(async () => {
      const result = await commitWorkflowImport({
        draftId: plan.draftId,
        action,
        workflowId: action === "NEW_VERSION" || action === "NEW_RECIPE" ? plan.existingWorkflowId : undefined,
        setCurrent: action === "NEW_VERSION",
      });
      const publishedResult = result as typeof result & { workflowId?: string; recipeId?: string };
      const workflowId = publishedResult.workflowId ?? plan.existingWorkflowId ?? plan.metadata.workflowId;
      const recipeId = publishedResult.recipeId ?? plan.metadata.recipeId;
      onPublished({ workflowId, recipeId });
      setPlan({
        ...plan,
        state: "AUTO_PUBLISHED",
        commitRequired: false,
        published: {
          ...result,
          workflowId,
          recipeId,
          workflowVersion: result.workflowVersion ?? plan.metadata.workflowVersion,
          packageName: result.packageName ?? plan.metadata.name,
          workflowSha256: result.workflowSha256 ?? plan.workflowSha256,
          refreshed: result.refreshed ?? { packagesFound: 0, valid: 0, invalid: 0, inserted: 0, reused: 0, errors: [] },
        },
      });
      setNotice(action === "NEW_VERSION" ? "已添加为新版本；已有项目绑定保持不变。" : "工作流已添加到工作流库。只有明确点击添加后才会写入。" );
      await onLoadWorkspace("refresh");
      await onCatalogChanged();
    });
  }, [onCatalogChanged, onLoadWorkspace, onPublished, plan, runDraftAction, setNotice]);

  const openAdvanced = useCallback(async () => {
    let nextDraft: WorkflowOnboardingDraftView | undefined;
    if (plan) {
      try {
        nextDraft = await getOnboardingDraft(plan.draftId);
      } catch (actionError: unknown) {
        setError(toUserMessage(actionError));
      }
    }
    onAdvancedRequested(nextDraft);
  }, [onAdvancedRequested, plan, setError]);

  const openExistingVersion = useCallback(async () => {
    if (!plan?.existingWorkflowId || !plan.existingWorkflowVersion) return;
    await runDraftAction(async () => {
      const currentDraft = draft ?? await getOnboardingDraft(plan.draftId);
      const nextDraft = await setOnboardingMetadata(plan.draftId, {
        workflowId: plan.existingWorkflowId!,
        name: currentDraft.manifest.name,
        workflowVersion: nextWorkflowVersion(plan.existingWorkflowVersion!),
        recipeVersion: "1.0.0",
        category: currentDraft.manifest.category,
        mode: currentDraft.manifest.mode,
      });
      onAdvancedRequested(nextDraft);
      setNotice("已选择添加为现有工作流的新版本。请在高级编辑中确认映射后发布；旧版本不会被覆盖。");
    });
  }, [draft, onAdvancedRequested, plan, runDraftAction, setNotice]);

  const openExisting = useCallback(async () => {
    if (!plan?.existingWorkflowId) return;
    const existing = workspaceItems.find((item) =>
      item.workflowId === plan.existingWorkflowId
      && (!plan.existingWorkflowVersion || item.workflowVersion === plan.existingWorkflowVersion),
    );
    const currentVersion = existing?.versions.find((version) => version.workflowVersionId === existing.currentVersionId)
      ?? existing?.versions[0];
    const recipe = existing?.currentRecipe
      ?? currentVersion?.recipes?.[currentVersion.recipes.length - 1]
      ?? existing?.recipes[existing.recipes.length - 1];
    if (!recipe) {
      setNotice("该工作流已经导入，请在工作流列表中查看现有版本。");
      return;
    }
    await onOpenStudio(plan.existingWorkflowId, recipe.recipeId);
  }, [onOpenStudio, plan, setNotice, workspaceItems]);

  const reidentify = useCallback(async (item: WorkflowProductionWorkspaceView) => {
    if (!item.workflowId || !item.workflowVersion || item.archived) return;
    await runDraftAction(async () => {
      const nextPlan = await rerecognizeWorkflow(item.workflowId!);
      setPlan(nextPlan);
      setImportError(undefined);
      onCloseAdvanced();
      setDraft(await getOnboardingDraft(nextPlan.draftId));
      setNotice(nextPlan.message || "已重新识别当前版本；请明确选择添加新 Recipe 后再写入。");
    });
  }, [onCloseAdvanced, runDraftAction, setDraft, setNotice]);

  return {
    plan,
    importError,
    smartImport,
    resume,
    resolveIssue,
    regenerateRecipe,
    commit,
    openAdvanced,
    openExisting,
    openExistingVersion,
    reidentify,
    resetSession,
  };
}

const fieldTypes: WorkflowFieldType[] = [
  "textarea",
  "integer",
  "number",
  "seed",
  "image",
  "images",
  "video",
  "videos",
  "audio",
  "audios",
];

function defaultInputMapping(nodeId: string, input: WorkflowInputView) {
  const safeName = input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "value";
  const fieldType = input.suggestedType && fieldTypes.includes(input.suggestedType as WorkflowFieldType)
    ? input.suggestedType as WorkflowFieldType
    : "textarea" as const;
  return {
    semanticKey: input.suggestedSemanticKey ?? `input_${nodeId}_${safeName}`,
    fieldType,
    label: fieldLabel(input.name),
    required: true,
    defaultValue: !input.isLinked && (fieldType === "textarea" || fieldType === "integer" || fieldType === "number" || fieldType === "seed")
      ? input.currentValueSummary === "random" ? "" : input.currentValueSummary
      : "",
    minValue: input.numericMin ?? "",
    maxValue: input.numericMax ?? "",
    minItems: "",
    maxItems: "",
    itemIndex: "",
  };
}

function optionalText(value: string): string | undefined {
  return value.trim() || undefined;
}

function optionalNumber(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined;
}

function fieldLabel(value: string): string {
  return value
    .split(/[_-]+/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
