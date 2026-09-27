import { useEffect, useMemo, useRef, useState } from "react";
import {
  createWorkflowExecution,
  getProductionQueue,
  getTaskDetail,
  getWorkflowExecutionSummary,
  getWorkflowRecipeHistory,
  preflightWorkflowExecution,
  startProductionQueue,
} from "../../services/tauriClient";
import { defaultGenerationValues } from "../../stores/studioStore";
import type { DraftValue, GenerationValues, RecipeViewModel } from "../../types/generation";
import type { ExecutionSummary, ExecutionValueSource, ProductionBatchDetail } from "../../types/productionQueue";
import type { TaskDetail } from "../../types/history";
import type { WorkflowSavedVersionDetailsView } from "../../types/workflowOnboarding";
import { errorMessageForCode, toUserMessage } from "../../i18n/errorMessages";
import { DynamicFormRenderer, validateRecipeValues } from "../studio/DynamicFormRenderer";

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
        key={recipe.recipeId}
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
  const [checking, setChecking] = useState(false);
  const [batchId, setBatchId] = useState<string>();
  const [batch, setBatch] = useState<ProductionBatchDetail>();
  const [task, setTask] = useState<TaskDetail>();
  const [previousTask, setPreviousTask] = useState<TaskDetail>();
  const [previousSummary, setPreviousSummary] = useState<ExecutionSummary>();
  const [error, setError] = useState<string>();
  const submissionKey = useRef(crypto.randomUUID());
  const busy = useRef(false);
  const validationErrors = validateRecipeValues(recipe, values);
  // Persisted recipe bindings are authoritative, including legacy versions
  // without V3 provenance. The backend compiler checks every binding target.
  const missingMappings = details.recognition?.issueCodes.filter((code) =>
    code === "INPUT_MAPPING_UNRESOLVED" || code === "AMBIGUOUS_INPUT") ?? [];

  useEffect(() => {
    let active = true;
    setPreviousTask(undefined);
    setPreviousSummary(undefined);
    void getWorkflowRecipeHistory(details.workflowVersionId, recipe.recipeId, undefined, 20)
      .then((history) => history.taskPage.items.find((item) =>
        item.projectId === projectId && (item.status === "SUCCEEDED" || item.status === "FAILED")
      ))
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
    if (!batchId) return;
    let active = true;
    const refresh = async () => {
      try {
        const next = await getProductionQueue(projectId, batchId);
        if (!active) return;
        setBatch(next);
        const item = next.items[0];
        if (item?.executionSummary) setPreviousSummary(item.executionSummary);
        if (item?.taskId && (item.status === "SUCCEEDED" || item.status === "FAILED" || item.status === "CANCELLED")) {
          setTask(await getTaskDetail(projectId, item.taskId));
        }
      } catch (cause: unknown) {
        if (active) setError(toUserMessage(cause));
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1500);
    return () => { active = false; window.clearInterval(timer); };
  }, [batchId, projectId]);

  function changeValue(key: string, value?: DraftValue) {
    setValues((current) => {
      const next = { ...current };
      if (value) next[key] = value;
      else delete next[key];
      return next;
    });
    setPreflight(undefined);
    setEditedFields((current) => new Set(current).add(key));
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
    try {
      if (!(await check())) return;
      const inputSources = Object.fromEntries(recipe.fields.map((field) => [
        field.key,
        values[field.key]?.type === "seed_random"
          ? "RUNTIME_RESOLVED"
          : editedFields.has(field.key) ? "USER_INPUT" : "WORKFLOW_DEFAULT",
      ])) as Record<string, ExecutionValueSource>;
      let id = batchId;
      if (!id) {
        const created = await createWorkflowExecution({
          projectId,
          workflowVersionId: details.workflowVersionId,
          recipeId: recipe.recipeId,
          values,
          submissionIdempotencyKey: submissionKey.current,
          inputSources,
        });
        id = created.id;
        setBatchId(id);
        setBatch(created);
        setPreviousSummary(created.items[0]?.executionSummary);
      }
      await startProductionQueue(projectId, id);
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      busy.current = false;
      setChecking(false);
    }
  }

  return <div>
    <p>版本 {details.workflowVersionId} · 配方 {recipe.recipeId} · Schema 来源：LIVE_COMFYUI</p>
    <DynamicFormRenderer recipe={recipe} values={values} validationErrors={validationErrors}
      onChange={changeValue} onGenerate={() => undefined} projectId={projectId} />
    <ul aria-label="执行参数来源">{recipe.fields.map((field) => <li key={field.key}>{field.key} → {values[field.key]?.type === "seed_random" ? "RUNTIME_RESOLVED" : editedFields.has(field.key) ? "USER_INPUT" : "WORKFLOW_DEFAULT"}</li>)}</ul>
    {!!missingMappings.length && <p role="alert">INPUT_MAPPING_UNRESOLVED · 请返回映射检查：{missingMappings.join(", ")}</p>}
    <div className="workflow-saved-details-actions">
      <button type="button" onClick={() => void (async () => {
        setChecking(true);
        setError(undefined);
        try { await check(); } catch (cause: unknown) { setError(toUserMessage(cause)); }
        finally { setChecking(false); }
      })()} disabled={checking || !comfyConnected || !!batchId}>运行前检查</button>
      <button type="button" onClick={() => void run()} disabled={checking || !comfyConnected || !!missingMappings.length || !!batchId && batch?.status !== "READY"}>运行工作流</button>
    </div>
    {preflight && <p role="status">Runtime Preflight: {preflight.status}{preflight.code ? ` · ${preflight.code}` : ""}{preflight.target ? ` · ${preflight.target}` : ""}</p>}
    {error && <p role="alert">{error}</p>}
    {batch && <div role="status">生产队列 {batch.id} · 执行配置 {batch.items[0]?.id} · {batch.items[0]?.status ?? batch.status}
      {batch.items[0]?.taskId && <button type="button" className="quiet-button" onClick={() => onOpenTask?.(batch.items[0].taskId!)}>打开任务详情</button>}
    </div>}
    {batch?.items[0]?.executionSummary && <ExecutionSummaryCard summary={batch.items[0].executionSummary} />}
    {task && <div role="status">任务 {task.id} · {task.status} · 结果资产 {task.outputAssets.length} 项
      <ul>{task.outputAssets.map((asset) => <li key={asset.id}>{asset.id} · {asset.name} · AI_STUDIO_MANAGED</li>)}</ul>
    </div>}
    {previousTask && <div role="status">历史执行 {previousTask.id} · {previousTask.status} · 结果资产 {previousTask.outputAssets.length} 项
      {onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(previousTask.id)}>重新打开结果</button>}
      {previousTask.status === "FAILED" && <p>{previousTask.errorCode ?? "GENERATION_FAILED"} · {errorMessageForCode(previousTask.errorCode ?? "GENERATION_FAILED")}</p>}
      {previousSummary && <ExecutionSummaryCard summary={previousSummary} />}
      <ul>{previousTask.outputAssets.map((asset) => <li key={asset.id}>{asset.id} · {asset.name} · AI_STUDIO_MANAGED</li>)}</ul>
    </div>}
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
