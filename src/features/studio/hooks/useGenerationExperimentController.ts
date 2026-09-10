import { useCallback, useEffect, useState } from "react";
import { createProductionQueue, startProductionQueue } from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import { formatDateTime, workflowDisplayName } from "../../../i18n/statusLabels";
import { useStudioStore } from "../../../stores/studioStore";
import type { WorkflowBenchmarkView } from "../../../types/benchmark";
import type { ReusableGenerationDraft } from "../../../types/history";
import type { PromptVersionView } from "../../../types/prompt";
import type { RecipeViewModel } from "../../../types/generation";
import type { ExperimentContext, ExperimentDimension, ExperimentPlan } from "../../experiments/experimentPlanner";
import type { StudioMode } from "../StudioModeTabs";
import { cloneGenerationValues } from "../batchDraft";

interface ExperimentWinnerSource {
  batchName: string;
  taskId: string;
}

export interface UseGenerationExperimentControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
  productCatalog: RecipeViewModel[];
  canExperimentBase: boolean;
  blockedReason?: string;
  onNotice: (message: string | null) => void;
  onProductionAdmissionChanged: () => Promise<void>;
  onStudioModeChange: (mode: StudioMode) => void;
  onClearMissingAssetFields: () => void;
}

export function useGenerationExperimentController({
  projectId,
  selectedWorkflow,
  productCatalog,
  canExperimentBase,
  blockedReason,
  onNotice,
  onProductionAdmissionChanged,
  onStudioModeChange,
  onClearMissingAssetFields,
}: UseGenerationExperimentControllerOptions) {
  const [experimentFocusBatchId, setExperimentFocusBatchId] = useState<string>();
  const [experimentContexts, setExperimentContexts] = useState<Record<string, ExperimentContext>>({});
  const [promptExperimentDimensions, setPromptExperimentDimensions] = useState<ExperimentDimension[]>([]);

  useEffect(() => {
    setExperimentFocusBatchId(undefined);
    setExperimentContexts({});
    setPromptExperimentDimensions([]);
  }, [projectId]);

  const focusBatch = useCallback((batchId?: string) => {
    setExperimentFocusBatchId(batchId);
  }, []);

  const clearFocusBatch = useCallback(() => {
    setExperimentFocusBatchId(undefined);
  }, []);

  const submitExperimentPlan = useCallback(async (plan: ExperimentPlan) => {
    if (!selectedWorkflow) return;
    if (!canExperimentBase) {
      onNotice(blockedReason ?? "当前基础草稿尚未满足实验队列提交条件。");
      return;
    }
    onNotice(null);
    try {
      const created = await createProductionQueue({
        projectId,
        name: `实验 · ${workflowDisplayName(selectedWorkflow.workflowId, selectedWorkflow.name)} · ${formatDateTime(new Date().toISOString())}`,
        continueOnFailure: true,
        items: plan.items.map((item) => ({
          workflowVersionId: plan.workflowVersionId,
          recipeId: plan.recipeId,
          values: cloneGenerationValues(item.values),
        })),
      });
      setExperimentContexts((current) => ({
        ...current,
        [created.id]: {
          recipe: selectedWorkflow,
          baseValues: cloneGenerationValues(plan.baseValues),
        },
      }));
      setExperimentFocusBatchId(created.id);
      onStudioModeChange("batch");
      try {
        await onProductionAdmissionChanged();
      } catch {
        // The queue is already persisted; a status refresh failure must not
        // prevent the normal queue runner from being started.
      }
      try {
        await startProductionQueue(projectId, created.id);
        onNotice(`实验队列已加入并开始执行，共 ${created.total} 项；任务将严格按顺序运行。`);
      } catch (startError: unknown) {
        onNotice(`实验队列已保存，共 ${created.total} 项；开始执行失败：${toUserMessage(startError)}。可在生产队列中手动开始。`);
      }
    } catch (error: unknown) {
      onNotice(toUserMessage(error));
    }
  }, [blockedReason, canExperimentBase, onNotice, onProductionAdmissionChanged, onStudioModeChange, projectId, selectedWorkflow]);

  const promoteExperimentWinner = useCallback(async (
    draft: ReusableGenerationDraft,
    source: ExperimentWinnerSource,
  ) => {
    if (draft.projectId !== projectId) {
      onNotice("生产结果属于其他项目，无法加载到当前创作。");
      return;
    }
    const workflow = productCatalog.find(
      (recipe) => recipe.workflowVersionId === draft.workflowVersionId && recipe.recipeId === draft.recipeId,
    );
    if (!workflow) {
      onNotice("生产结果对应的工作流版本已不在运行目录中，请先刷新工作流列表。");
      return;
    }
    useStudioStore.getState().loadDraft(workflow, cloneGenerationValues(draft.values));
    useStudioStore.getState().setReuseProvenance({
      workflowName: draft.workflowName,
      createdAt: draft.createdAt,
      sourceBatchName: source.batchName,
      sourceTaskId: source.taskId,
    });
    onClearMissingAssetFields();
    onStudioModeChange("single");
    onNotice(draft.missingAssetIds.length
      ? "已将生产结果加载到 Studio，但部分素材缺失，请替换后再生成；未自动提交任务。"
      : "已将生产结果作为下一轮起点加载到 Studio，未自动提交生成任务。",
    );
  }, [onClearMissingAssetFields, onNotice, onStudioModeChange, productCatalog, projectId]);

  const usePromptVersionsForExperiment = useCallback((fieldKey: string, versions: PromptVersionView[]) => {
    setPromptExperimentDimensions([{
      fieldKey,
      values: versions.map((version) => ({ type: "string", value: version.text })),
    }]);
    onStudioModeChange("experiment");
  }, [onStudioModeChange]);

  const handleBenchmarkCreated = useCallback((created: WorkflowBenchmarkView) => {
    setExperimentFocusBatchId(created.productionBatchId);
    onNotice(`基准实验已保存：${created.candidates.length} 个候选已进入普通生产队列。`);
  }, [onNotice]);

  return {
    experimentFocusBatchId,
    experimentContexts,
    promptExperimentDimensions,
    clearPromptExperimentDimensions: () => setPromptExperimentDimensions([]),
    focusBatch,
    clearFocusBatch,
    submitExperimentPlan,
    promoteExperimentWinner,
    usePromptVersionsForExperiment,
    handleBenchmarkCreated,
  };
}
