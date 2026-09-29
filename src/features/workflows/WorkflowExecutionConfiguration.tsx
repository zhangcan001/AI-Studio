import { useEffect, useMemo, useRef, useState } from "react";
import {
  createWorkflowExecution,
  createWorkflowExecutionBatch,
  cancelPendingProductionQueue,
  getProductionQueue,
  getTaskDetail,
  getWorkflowExecutionSummary,
  getWorkflowRecipeHistory,
  preflightWorkflowExecution,
  startProductionQueue,
} from "../../services/tauriClient";
import { defaultGenerationValues } from "../../stores/studioStore";
import type { DraftValue, GenerationValues, RecipeViewModel } from "../../types/generation";
import type { ExecutionSummary, ExecutionValueSource, ProductionBatchDetail, ProductionBatchPreflightIssue } from "../../types/productionQueue";
import type { TaskDetail } from "../../types/history";
import type { WorkflowSavedVersionDetailsView } from "../../types/workflowOnboarding";
import { errorMessageForCode, productionBatchPreflightIssues, toUserMessage } from "../../i18n/errorMessages";
import { formatDurationMs, productionItemStatusLabel, productionStatusLabel } from "../../i18n/statusLabels";
import { DynamicFormRenderer, validateRecipeValues } from "../studio/DynamicFormRenderer";
import { canCancelPendingProductionQueue } from "../studio/productionQueuePolicy";
import { productionBatchOutcomeLabel } from "../studio/productionQueueOutcome";
import { AssetCard } from "../assets/AssetCard";
import { AssetPickerDialog } from "../studio/AssetPickerDialog";
import { AssetPreview } from "../assets/AssetPreview";
import { ComfyNodeErrorSection } from "../../components/tasks/ComfyNodeErrorSection";
import {
  MAX_WORKFLOW_EXECUTION_BATCH_ITEMS,
  materializeWorkflowExecutionBatch,
  workflowExecutionBatchFields,
} from "./workflowExecutionBatch";

interface Props {
  details: WorkflowSavedVersionDetailsView;
  catalog: RecipeViewModel[];
  projectId?: string;
  comfyConnected: boolean;
  onOpenTask?: (taskId: string) => void;
}

export function WorkflowExecutionConfiguration({ details, catalog, projectId, comfyConnected, onOpenTask }: Props) {
  const recipes = useMemo(() => catalog.filter((recipe) => (
    recipe.workflowId === details.workflowId && recipe.workflowVersionId === details.workflowVersionId
  )), [catalog, details.workflowId, details.workflowVersionId]);
  const [recipeId, setRecipeId] = useState(recipes.length === 1 ? recipes[0].recipeId : "");
  useEffect(() => {
    setRecipeId((current) => {
      if (current && recipes.some((candidate) => candidate.recipeId === current)) return current;
      return recipes.length === 1 ? recipes[0].recipeId : "";
    });
  }, [recipes]);
  const recipe = recipes.find((item) => item.recipeId === recipeId);
  return (
    <section className="workflow-saved-details workflow-diff-panel" aria-label="执行配置">
      <h3>运行工作流 · 执行配置</h3>
      <p>本次参数只属于执行实例，不会修改已保存的 WorkflowVersion。</p>
      {!projectId && <p role="status">先选择项目，才能运行工作流。</p>}
      {!comfyConnected && <p role="status">COMFYUI_CONNECTION_UNAVAILABLE · 可以配置参数，但当前不可运行。</p>}
      {!recipes.length && <p role="status">此版本暂无可用配方；请先恢复或发布配方。</p>}
      {recipes.length > 0 && <label>配方
        <select value={recipeId} onChange={(event) => setRecipeId(event.target.value)}>
          <option value="">选择精确配方</option>
          {recipes.map((item) => <option key={item.recipeId} value={item.recipeId}>{item.recipeVersion ?? "版本未知"} · {item.recipeId}</option>)}
        </select>
      </label>}
      {recipe && projectId && <ExecutionForm
        key={`${projectId}:${details.workflowVersionId}:${recipe.recipeId}`}
        details={details}
        recipe={recipe}
        projectId={projectId}
        comfyConnected={comfyConnected}
        onOpenTask={onOpenTask}
      />}
    </section>
  );
}

function ExecutionForm({ details, recipe, projectId, comfyConnected, onOpenTask }: {
  details: WorkflowSavedVersionDetailsView;
  recipe: RecipeViewModel;
  projectId: string;
  comfyConnected: boolean;
  onOpenTask?: (taskId: string) => void;
}) {
  const [values, setValues] = useState<GenerationValues>(() => defaultGenerationValues(recipe));
  const [editedFields, setEditedFields] = useState<Set<string>>(() => new Set());
  const [preflight, setPreflight] = useState<{ status: string; code?: string; target?: string }>();
  const [structuredIssues, setStructuredIssues] = useState<ProductionBatchPreflightIssue[]>([]);
  const [checking, setChecking] = useState(false);
  const [batchId, setBatchId] = useState<string>();
  const [batch, setBatch] = useState<ProductionBatchDetail>();
  const [tasksByItem, setTasksByItem] = useState<Record<string, TaskDetail>>({});
  const [previewAsset, setPreviewAsset] = useState<TaskDetail["outputAssets"][number]>();
  const [batchConfigurationOpen, setBatchConfigurationOpen] = useState(false);
  const [promptBatchInput, setPromptBatchInput] = useState("");
  const [seedCount, setSeedCount] = useState(1);
  const [batchImageAssetIds, setBatchImageAssetIds] = useState<string[]>([]);
  const [imageBatchPickerOpen, setImageBatchPickerOpen] = useState(false);
  const [previousTask, setPreviousTask] = useState<TaskDetail>();
  const [previousSummary, setPreviousSummary] = useState<ExecutionSummary>();
  const [error, setError] = useState<string>();
  const submissionKey = useRef(crypto.randomUUID());
  const busy = useRef(false);
  const loadedTaskIds = useRef(new Set<string>());
  const batchFields = useMemo(() => workflowExecutionBatchFields(recipe), [recipe]);
  const hasBatchOverrides = Boolean(promptBatchInput.trim() || seedCount > 1 || batchImageAssetIds.length);
  const materializedBatch = useMemo(() => materializeWorkflowExecutionBatch(
    recipe,
    values,
    promptBatchInput,
    seedCount,
    batchImageAssetIds,
  ), [batchImageAssetIds, promptBatchInput, recipe, seedCount, values]);
  const validationErrors = validateRecipeValues(recipe, values);
  const batchValidationIssues = useMemo(() => materializedBatch.items.flatMap((item, index) => (
    Object.keys(validateRecipeValues(recipe, item.values)).map((field) => ({ index, field }))
  )), [materializedBatch.items, recipe]);
  // Persisted recipe bindings are authoritative, including legacy versions
  // without V3 provenance. The backend compiler checks every binding target.
  const missingMappings = details.recognition?.issueCodes.filter((code) =>
    code === "INPUT_MAPPING_UNRESOLVED" || code === "AMBIGUOUS_INPUT") ?? [];

  useEffect(() => {
    let active = true;
    setPreviousTask(undefined);
    setPreviousSummary(undefined);
    void getWorkflowRecipeHistory(
      details.workflowVersionId,
      recipe.recipeId,
      undefined,
      20,
      projectId,
      ["SUCCEEDED", "FAILED"],
    )
      .then((history) => history.taskPage.items[0])
      .then(async (item) => {
        if (!item || !projectId) return undefined;
        const [previous, summary] = await Promise.all([
          getTaskDetail(projectId, item.id),
          getWorkflowExecutionSummary(projectId, item.id),
        ]);
        return { previous, summary };
      })
      .then((result) => {
        if (!active || !result) return;
        setPreviousTask(result.previous);
        setPreviousSummary(result.summary ?? undefined);
      })
      .catch(() => undefined);
    return () => { active = false; };
  }, [details.workflowVersionId, recipe.recipeId, projectId]);

  useEffect(() => {
    loadedTaskIds.current.clear();
    setTasksByItem({});
  }, [batchId]);

  useEffect(() => {
    if (!batchId) return;
    let active = true;
    let timer: number | undefined;
    let inFlight = false;
    let completedDetailRetries = 0;
    let completedObserved = false;
    const schedule = (delay: number) => {
      if (active) timer = window.setTimeout(() => void refresh(), delay);
    };
    const refresh = async () => {
      if (!active || inFlight) return;
      inFlight = true;
      let nextDelay: number | undefined = 1500;
      try {
        const next = await getProductionQueue(projectId, batchId);
        if (!active) return;
        setBatch(next);
        const item = next.items[0];
        if (item?.executionSummary) setPreviousSummary(item.executionSummary);
        const terminalItems = next.items.filter((candidate) => candidate.taskId
          && (candidate.status === "SUCCEEDED" || candidate.status === "FAILED" || candidate.status === "CANCELLED"
            || candidate.status === "SKIPPED")
          && !loadedTaskIds.current.has(candidate.taskId));
        const taskDetails = await Promise.all(terminalItems.map(async (candidate) => {
          const taskId = candidate.taskId!;
          loadedTaskIds.current.add(taskId);
          try {
            return [candidate.id, await getTaskDetail(projectId, taskId)] as const;
          } catch {
            loadedTaskIds.current.delete(taskId);
            return undefined;
          }
        }));
        const loadedDetails = taskDetails.filter((entry): entry is NonNullable<typeof entry> => entry !== undefined);
        if (active && loadedDetails.length) {
          setTasksByItem((current) => ({ ...current, ...Object.fromEntries(loadedDetails) }));
        }
        const missingTerminalDetails = terminalItems.length - loadedDetails.length;
        if (next.status === "COMPLETED") {
          completedObserved = true;
          if (missingTerminalDetails > 0 && completedDetailRetries < 3) {
            completedDetailRetries += 1;
            nextDelay = 1500;
          } else {
            nextDelay = undefined;
          }
        }
      } catch (cause: unknown) {
        if (active) {
          setStructuredIssues(productionBatchPreflightIssues(cause));
          setError(toUserMessage(cause));
          if (completedObserved && completedDetailRetries >= 3) nextDelay = undefined;
        }
      } finally {
        inFlight = false;
        if (active && nextDelay !== undefined) schedule(nextDelay);
      }
    };
    void refresh();
    return () => {
      active = false;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [batchId, projectId]);

  function changeValue(key: string, value?: DraftValue) {
    setValues((current) => {
      const next = { ...current };
      if (value) next[key] = value;
      else delete next[key];
      return next;
    });
    setPreflight(undefined);
    setStructuredIssues([]);
    setEditedFields((current) => new Set(current).add(key));
    submissionKey.current = crypto.randomUUID();
  }

  function changePromptBatch(value: string) {
    setPromptBatchInput(value);
    setPreflight(undefined);
    setStructuredIssues([]);
    submissionKey.current = crypto.randomUUID();
  }

  function changeSeedCount(value: string) {
    const next = Number(value);
    setSeedCount(Number.isFinite(next) ? Math.trunc(next) : 0);
    setPreflight(undefined);
    setStructuredIssues([]);
    submissionKey.current = crypto.randomUUID();
  }

  function changeBatchImages(assetIds: string[]) {
    setBatchImageAssetIds(assetIds);
    setImageBatchPickerOpen(false);
    setPreflight(undefined);
    setStructuredIssues([]);
    submissionKey.current = crypto.randomUUID();
  }

  async function check(): Promise<boolean> {
    if (!comfyConnected) {
      setPreflight({ status: "BLOCKED", code: "COMFYUI_CONNECTION_UNAVAILABLE" });
      return false;
    }
    if (missingMappings.length) {
      setPreflight({ status: "NEEDS_REVIEW", code: "INPUT_MAPPING_UNRESOLVED" });
      return false;
    }
    if (hasBatchOverrides) {
      if (materializedBatch.errors.length) {
        setPreflight({ status: "BLOCKED", code: materializedBatch.errors[0] });
        return false;
      }
      if (!materializedBatch.items.length || batchValidationIssues.length) {
        setPreflight({ status: "BLOCKED", code: "EXECUTION_INPUT_INVALID" });
        return false;
      }
      setPreflight({ status: "READY", code: "BATCH_RUNTIME_PREFLIGHT_ON_START" });
      return true;
    }
    if (Object.keys(validationErrors).length) {
      setPreflight({ status: "BLOCKED", code: "EXECUTION_INPUT_INVALID" });
      return false;
    }
    const result = await preflightWorkflowExecution({
      projectId, workflowVersionId: details.workflowVersionId, recipeId: recipe.recipeId, values,
    });
    setPreflight(result);
    return result.status === "READY";
  }

  async function run() {
    if (busy.current) return;
    busy.current = true;
    setChecking(true);
    setError(undefined);
    setStructuredIssues([]);
    try {
      if (!(await check())) return;
      let id = batchId;
      if (!id) {
        const created = hasBatchOverrides
          ? await createWorkflowExecutionBatch({
            projectId,
            name: `${details.name} · 批量执行`,
            workflowVersionId: details.workflowVersionId,
            recipeId: recipe.recipeId,
            submissionIdempotencyKey: submissionKey.current,
            items: materializedBatch.items.map(({ values: itemValues }) => ({
              values: itemValues,
              inputSources: Object.fromEntries(recipe.fields.map((field) => {
                const value = itemValues[field.key];
                const overridden = editedFields.has(field.key)
                  || field.key === batchFields.prompt?.key && Boolean(promptBatchInput.trim())
                  || field.key === batchFields.seed?.key && seedCount > 1
                  || field.key === batchFields.image?.key && batchImageAssetIds.length > 0;
                const source: ExecutionValueSource = value?.type === "seed_random"
                  ? "RUNTIME_RESOLVED"
                  : overridden ? "USER_INPUT" : "WORKFLOW_DEFAULT";
                return [field.key, source];
              })) as Record<string, ExecutionValueSource>,
            })),
          })
          : await createWorkflowExecution({
            projectId,
            workflowVersionId: details.workflowVersionId,
            recipeId: recipe.recipeId,
            values,
            submissionIdempotencyKey: submissionKey.current,
            inputSources: Object.fromEntries(recipe.fields.map((field) => [
              field.key,
              values[field.key]?.type === "seed_random"
                ? "RUNTIME_RESOLVED"
                : editedFields.has(field.key) ? "USER_INPUT" : "WORKFLOW_DEFAULT",
            ])) as Record<string, ExecutionValueSource>,
          });
        id = created.id;
        setBatchId(id);
        setBatch(created);
        setPreviousSummary(created.items[0]?.executionSummary);
      }
      await startProductionQueue(projectId, id);
    } catch (cause: unknown) {
      setStructuredIssues(productionBatchPreflightIssues(cause));
      setError(toUserMessage(cause));
    } finally {
      busy.current = false;
      setChecking(false);
    }
  }

  async function cancelBatch() {
    if (!batchId || busy.current || !batch) return;
    const hasActiveItem = batch.running > 0;
    const message = hasActiveItem
      ? "取消批次剩余项目？当前已派发任务会继续运行，队列不会再派发新任务。"
      : "取消批次尚未派发的项目？这些项目将标记为已取消。";
    if (!window.confirm(message)) return;
    busy.current = true;
    setChecking(true);
    setError(undefined);
    setStructuredIssues([]);
    try {
      setBatch(await cancelPendingProductionQueue(projectId, batchId));
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      busy.current = false;
      setChecking(false);
    }
  }

  function prepareRerun() {
    setBatchId(undefined);
    setBatch(undefined);
    setTasksByItem({});
    loadedTaskIds.current.clear();
    setError(undefined);
    setPreflight(undefined);
    setStructuredIssues([]);
    setPreviousTask(undefined);
    setPreviousSummary(undefined);
    submissionKey.current = crypto.randomUUID();
  }

  const hiddenBatchFields = new Set<string>();
  if (promptBatchInput.trim() && batchFields.prompt) hiddenBatchFields.add(batchFields.prompt.key);
  if (seedCount > 1 && batchFields.seed) hiddenBatchFields.add(batchFields.seed.key);
  if (batchImageAssetIds.length && batchFields.image) hiddenBatchFields.add(batchFields.image.key);
  const formRecipe = hiddenBatchFields.size
    ? { ...recipe, fields: recipe.fields.filter((field) => !hiddenBatchFields.has(field.key)) }
    : recipe;
  const promptLines = promptBatchInput.split(/\r?\n/).filter((line) => line.trim()).length;
  const batchReadyToSubmit = materializedBatch.items.length > 0
    && materializedBatch.errors.length === 0
    && batchValidationIssues.length === 0;
  const terminalCount = batch ? batch.succeeded + batch.failed + batch.cancelled + batch.skipped : 0;
  const batchProgress = batch?.total ? Math.round((terminalCount / batch.total) * 100) : 0;
  const batchStatusLabel = batch?.status === "COMPLETED"
    ? productionBatchOutcomeLabel(batch)
    : batch ? productionStatusLabel(batch.status) : "";

  return <div>
    <p>版本 {details.workflowVersionId} · 配方 {recipe.recipeId} · Schema 来源：LIVE_COMFYUI</p>
    {!batchId && <>
      {(batchFields.prompt || batchFields.seed || batchFields.image) && <details
        className="workflow-execution-batch-config"
        open={batchConfigurationOpen || hasBatchOverrides}
        onToggle={(event) => setBatchConfigurationOpen(event.currentTarget.open)}
      >
        <summary>批量生成</summary>
        {batchFields.prompt && <label className="workflow-execution-batch-field">
          <span>Prompt 列表（每行一个）</span>
          <textarea aria-label="Prompt 列表（每行一个）" value={promptBatchInput} onChange={(event) => changePromptBatch(event.target.value)} rows={4} placeholder="留空时使用下方单个 Prompt。" />
        </label>}
        {batchFields.seed && <label className="workflow-execution-batch-field">
          <span>每组 Prompt 的 Seed 数量</span>
          <input aria-label="Seed 数量" type="number" min={1} max={MAX_WORKFLOW_EXECUTION_BATCH_ITEMS} step={1} value={seedCount} onChange={(event) => changeSeedCount(event.target.value)} />
          <small>随机 Seed 会在批次创建时冻结；固定 Seed 按连续值展开。</small>
        </label>}
        {batchFields.image && <div className="workflow-execution-batch-field">
          <span>输入图片列表</span>
          <button type="button" className="quiet-button" onClick={() => setImageBatchPickerOpen(true)}>选择图片（{batchImageAssetIds.length}）</button>
          {!!batchImageAssetIds.length && <ul aria-label="已选批量输入图片">{batchImageAssetIds.map((assetId) => <li key={assetId}>{assetId}</li>)}</ul>}
          <small>多张图片仅支持与一个 Prompt、一个 Seed 组合，避免意外笛卡尔积。</small>
        </div>}
        {!!(promptLines || seedCount > 1 || batchImageAssetIds.length) && <section className="workflow-execution-batch-preview" aria-label="批次预览">
          <strong>将创建 {materializedBatch.expectedCount} 个任务</strong>
          {!!materializedBatch.errors.length && <p role="alert">{materializedBatch.errors.join(" · ")}</p>}
          {!!batchValidationIssues.length && <p role="alert">批次输入无效：第 {batchValidationIssues[0].index + 1} 项的 {batchValidationIssues[0].field} 未通过校验。</p>}
          {!materializedBatch.errors.length && !batchValidationIssues.length && materializedBatch.expectedCount > 0 && <p>批次将使用当前保存版本与精确配方；启动时统一执行实时批次预检。</p>}
        </section>}
      </details>}
      <DynamicFormRenderer recipe={formRecipe} values={values} validationErrors={validationErrors}
        onChange={changeValue} onGenerate={() => undefined} projectId={projectId} />
      <ul aria-label="执行参数来源">{recipe.fields.map((field) => <li key={field.key}>{field.key} → {values[field.key]?.type === "seed_random" ? "RUNTIME_RESOLVED" : editedFields.has(field.key) ? "USER_INPUT" : "WORKFLOW_DEFAULT"}</li>)}</ul>
    </>}
    {!!missingMappings.length && <p role="alert">INPUT_MAPPING_UNRESOLVED · 请返回映射检查：{missingMappings.join(", ")}</p>}
    <div className="workflow-saved-details-actions">
      <button type="button" onClick={() => void (async () => {
        setChecking(true);
        setError(undefined);
        try { await check(); } catch (cause: unknown) {
          setStructuredIssues(productionBatchPreflightIssues(cause));
          setError(toUserMessage(cause));
        }
        finally { setChecking(false); }
      })()} disabled={checking || !comfyConnected || !!batchId || hasBatchOverrides && !batchReadyToSubmit}>{hasBatchOverrides ? "检查批次配置" : "运行前检查"}</button>
      <button type="button" onClick={() => void run()} disabled={checking || !comfyConnected || !!missingMappings.length || !!batchId && batch?.status !== "READY" && batch?.status !== "PAUSED" || !batchId && hasBatchOverrides && !batchReadyToSubmit}>{batchId ? batch?.status === "PAUSED" ? "继续批次" : "启动批次" : hasBatchOverrides ? "创建并启动批次" : "运行工作流"}</button>
    </div>
    {preflight && <p role="status">Runtime Preflight: {preflight.status}{preflight.code ? ` · ${preflight.code}` : ""}{preflight.target ? ` · ${preflight.target}` : ""}</p>}
    {!!structuredIssues.length && <section aria-label="结构化准入问题" className="workflow-execution-issues">
      <h4>启动前检查发现的问题</h4>
      <ul>{structuredIssues.map((issue) => <li key={`${issue.itemNumber}-${issue.code}-${issue.target ?? ""}`}>
        第 {issue.itemNumber} 项：{errorMessageForCode(issue.code)}
        {issue.semanticField && ` · 字段 ${issue.semanticField}`}
        {issue.nodeId && ` · 节点 ${issue.nodeId}`}
        {issue.inputName && ` · 输入 ${issue.inputName}`}
        {issue.target && ` · 目标 ${issue.target}`}
        {issue.messageArgs !== null && <code> · {JSON.stringify(issue.messageArgs)}</code>}
      </li>)}</ul>
    </section>}
    {error && <p role="alert">{error}</p>}
    {batch && <section className="workflow-execution-batch-results" aria-label="批次结果">
      <h4>批次 · {batch.name || batch.id}</h4>
      <p>状态 {batchStatusLabel} · 总计 {batch.total} · 等待 {batch.pending} · 运行中 {batch.running} · 成功 {batch.succeeded} · 失败 {batch.failed} · 取消 {batch.cancelled}</p>
      {canCancelPendingProductionQueue(batch) && <button type="button" className="quiet-button danger-button" onClick={() => void cancelBatch()} disabled={checking}>
        {batch.running > 0 ? "取消剩余项" : "取消待执行项"}
      </button>}
      {batch.status === "COMPLETED" && <button type="button" className="quiet-button" onClick={prepareRerun} disabled={checking}>再次执行</button>}
      <progress aria-label="批次完成进度" value={batchProgress} max={100}>{batchProgress}%</progress>
      <span>{batchProgress}%</span>
      <ol>{batch.items.map((item) => {
        const itemTask = tasksByItem[item.id];
        const promptSummary = item.promptText?.trim();
        return <li key={item.id} className="workflow-execution-batch-item">
          <div>
            <strong>任务 {item.ordinal + 1} · {productionItemStatusLabel(item.status)}</strong>
            {promptSummary && <p>{promptSummary.length > 140 ? `${promptSummary.slice(0, 140)}…` : promptSummary}</p>}
            <small>Seed {item.seed ?? "—"} · 输入图片 {item.inputAssetIds?.join(", ") || "无"}</small>
            <p>耗时 {formatDurationMs(itemTask?.telemetry?.totalMs)} · 结果 {itemTask?.outputAssets.length ?? 0}</p>
            {item.errorCode && <p role="alert">{item.errorCode} · {errorMessageForCode(item.errorCode)}</p>}
            {item.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>打开任务详情</button>}
          </div>
          {item.executionSummary && <details><summary>执行参数摘要</summary><ExecutionSummaryCard summary={item.executionSummary} /></details>}
          {!!itemTask?.outputAssets.length && <div className="workflow-execution-batch-asset-grid" aria-label={`第 ${item.ordinal + 1} 项结果资产`}>
            {itemTask.outputAssets.map((asset) => <AssetCard key={asset.id} projectId={projectId} asset={asset} onSelect={setPreviewAsset} />)}
          </div>}
          {itemTask && (itemTask.nodeErrors?.length || itemTask.rawError !== undefined) ? (
            <ComfyNodeErrorSection
              nodeErrors={itemTask.nodeErrors ?? []}
              rawError={itemTask.rawError}
              title={`第 ${item.ordinal + 1} 项节点错误详情`}
            />
          ) : null}
        </li>;
      })}</ol>
    </section>}
    {previousTask && <div role="status">历史执行 {previousTask.id} · {previousTask.status} · 结果资产 {previousTask.outputAssets.length} 项
      {onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(previousTask.id)}>重新打开结果</button>}
      {previousTask.status === "FAILED" && <p>{previousTask.errorCode ?? "GENERATION_FAILED"} · {errorMessageForCode(previousTask.errorCode ?? "GENERATION_FAILED")}</p>}
      {previousSummary && <ExecutionSummaryCard summary={previousSummary} />}
      <ul>{previousTask.outputAssets.map((asset) => <li key={asset.id}>{asset.id} · {asset.name} · AI_STUDIO_MANAGED</li>)}</ul>
    </div>}
    {imageBatchPickerOpen && batchFields.image && <AssetPickerDialog
      projectId={projectId}
      kind="image"
      multiple
      maxItems={MAX_WORKFLOW_EXECUTION_BATCH_ITEMS}
      selectedIds={batchImageAssetIds}
      onCancel={() => setImageBatchPickerOpen(false)}
      onConfirm={changeBatchImages}
    />}
    {previewAsset && <AssetPreview projectId={projectId} asset={previewAsset} onClose={() => setPreviewAsset(undefined)} onOpenTask={onOpenTask} />}
  </div>;
}

function ExecutionSummaryCard({ summary }: { summary: ExecutionSummary }) {
  return <section aria-label="持久化执行摘要">
    <h4>执行摘要 · {summary.preflightStatus}</h4>
    <p>Workflow {summary.workflowId} · 版本 {summary.workflowVersionId} ({summary.workflowVersion}) · Recipe {summary.recipeId} ({summary.recipeVersion})</p>
    <p>创建于 {summary.createdAt} · Schema {summary.runtimeSchemaSource}</p>
    <ul>{summary.inputs.map((input) => <li key={input.semanticField}>
      <strong>{input.semanticField}</strong> → {input.targets.map((target) => `${target.node}.${target.input}`).join(", ") || "无绑定"}
      ：{input.valueSummary} · {input.source}
    </li>)}</ul>
  </section>;
}
