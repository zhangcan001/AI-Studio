import { useEffect, useMemo, useState } from "react";
import {
  createProductionQueue,
  listModelVersions,
  listModels,
  listPromptLibrary,
  listToolInstances,
  listToolVersions,
  listTools,
} from "../../services/tauriClient";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type { ModelVersionView, ModelView } from "../../types/model";
import type { PromptEntryView } from "../../types/prompt";
import type { ToolInstanceView, ToolVersionView, ToolView } from "../../types/tool";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { ShotStage, ShotView } from "../../types/shot";
import { defaultGenerationValues } from "../../stores/studioStore";
import { toUserMessage } from "../../i18n/errorMessages";
import { DynamicFormRenderer, validateRecipeValues } from "../studio/DynamicFormRenderer";
import { migrateGenerationValues } from "../runtime/workflowCapabilities";
import "./DirectGenerationEntry.css";

interface ModelVersionOption {
  model: ModelView;
  version: ModelVersionView;
}

interface ToolRegistryEntry {
  tool: ToolView;
  instances: ToolInstanceView[];
  versions: ToolVersionView[];
}

export interface DirectGenerationEntryProps {
  projectId: string;
  projectName?: string;
  catalog: RecipeViewModel[];
  shots: ShotView[];
  enabled?: boolean;
  onOpenProductionQueue?: (batchId?: string) => void | Promise<void>;
}

export function DirectGenerationEntry({
  projectId,
  projectName,
  catalog,
  shots,
  enabled = true,
  onOpenProductionQueue,
}: DirectGenerationEntryProps) {
  const [selectedRecipeKey, setSelectedRecipeKey] = useState(() => recipeKey(catalog[0]));
  const [values, setValues] = useState<GenerationValues>(() => catalog[0] ? defaultGenerationValues(catalog[0]) : {});
  const [selectedShotId, setSelectedShotId] = useState("");
  const [stage, setStage] = useState<ShotStage>("image");
  const [name, setName] = useState("单次生成");
  const [promptVersionId, setPromptVersionId] = useState("");
  const [modelVersionId, setModelVersionId] = useState("");
  const [toolInstanceId, setToolInstanceId] = useState("");
  const [toolVersionId, setToolVersionId] = useState("");
  const [prompts, setPrompts] = useState<PromptEntryView[]>([]);
  const [modelVersions, setModelVersions] = useState<ModelVersionOption[]>([]);
  const [toolEntries, setToolEntries] = useState<ToolRegistryEntry[]>([]);
  const [metadataLoading, setMetadataLoading] = useState(false);
  const [metadataError, setMetadataError] = useState<string>();
  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [queueOpenFailed, setQueueOpenFailed] = useState(false);
  const [createdBatch, setCreatedBatch] = useState<ProductionBatchDetail>();

  const selectedRecipe = useMemo(
    () => catalog.find((recipe) => recipeKey(recipe) === selectedRecipeKey) ?? catalog[0],
    [catalog, selectedRecipeKey],
  );
  const selectedShot = shots.find((shot) => shot.id === selectedShotId);
  const selectedToolInstance = toolEntries
    .flatMap((entry) => entry.instances.map((instance) => ({ entry, instance })))
    .find(({ instance }) => instance.id === toolInstanceId);
  const availableToolVersions = selectedToolInstance?.entry.versions ?? [];

  useEffect(() => {
    const nextRecipe = catalog.find((recipe) => recipeKey(recipe) === selectedRecipeKey) ?? catalog[0];
    if (!nextRecipe) {
      setSelectedRecipeKey("");
      setValues({});
      return;
    }
    if (recipeKey(nextRecipe) !== selectedRecipeKey) {
      setSelectedRecipeKey(recipeKey(nextRecipe));
      setValues(defaultGenerationValues(nextRecipe));
      setValidationErrors({});
    }
  }, [catalog, selectedRecipeKey]);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setMetadataLoading(true);
    setMetadataError(undefined);
    void loadRegistryMetadata(projectId).then((result) => {
      if (!active) return;
      setPrompts(result.prompts);
      setModelVersions(result.modelVersions);
      setToolEntries(result.tools);
      if (result.errors.length) setMetadataError(`部分可选来源加载失败：${result.errors.join("；")}`);
    }).catch((loadError: unknown) => {
      if (active) setMetadataError(toUserMessage(loadError));
    }).finally(() => {
      if (active) setMetadataLoading(false);
    });
    return () => {
      active = false;
    };
  }, [enabled, projectId]);

  function updateValue(key: string, value: GenerationValues[string] | undefined) {
    setValues((current) => {
      const next = { ...current };
      if (value) next[key] = value;
      else delete next[key];
      return next;
    });
    setValidationErrors((current) => {
      if (!current[key]) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
  }

  async function submit() {
    if (!enabled || busy) return;
    setError(undefined);
    setNotice(undefined);
    setQueueOpenFailed(false);
    if (!selectedRecipe) {
      setError("当前没有可用的生成配方，请先完成工作流配置。");
      return;
    }
    const nextValidationErrors = validateRecipeValues(selectedRecipe, values);
    if (Object.keys(nextValidationErrors).length) {
      setValidationErrors(nextValidationErrors);
      setError("请先完成生成参数。");
      return;
    }
    setValidationErrors({});
    setBusy(true);
    try {
      const created = await createProductionQueue({
        projectId,
        name: name.trim() || "单次生成",
        continueOnFailure: true,
        direct: true,
        shotId: selectedShotId || undefined,
        stage: selectedShotId ? stage : undefined,
        promptVersionId: promptVersionId || undefined,
        modelVersionId: modelVersionId || undefined,
        toolInstanceId: toolInstanceId || undefined,
        toolVersionId: toolVersionId || undefined,
        items: [{
          workflowVersionId: selectedRecipe.workflowVersionId,
          recipeId: selectedRecipe.recipeId,
          values,
        }],
      });
      setCreatedBatch(created);
      setNotice("单次生成已进入现有生产队列，当前待启动；不会自动启动 Comfy。");
      if (onOpenProductionQueue) {
        try {
          await onOpenProductionQueue(created.id);
          setNotice("单次生成已进入生产队列；请在队列中明确点击“开始生产”。");
        } catch (openError: unknown) {
          setQueueOpenFailed(true);
          setError(`单次生成已创建，但生产队列暂时无法打开：${toUserMessage(openError)}`);
        }
      }
    } catch (createError: unknown) {
      setError(toUserMessage(createError));
    } finally {
      setBusy(false);
    }
  }

  if (!enabled) {
    return (
      <section className="direct-generation-entry" aria-label="单次生成入口" data-project-id={projectId}>
        <div className="direct-generation-entry-empty"><strong>单次生成</strong><p>切换到此页后，选择一个目标和明确的提示词、模型与工具上下文。</p></div>
      </section>
    );
  }

  return (
    <section className="direct-generation-entry" aria-label="单次生成入口" data-project-id={projectId} aria-busy={busy}>
      <header className="direct-generation-entry-heading">
        <div><span className="section-label">生产 · 单次入口</span><h3>单次生成</h3><p>为当前项目准备一个目标生成，保存后进入现有队列。</p></div>
        <span className="direct-generation-queue-badge">点击“开始生产”后才会执行</span>
      </header>

      <div className="direct-generation-scope-note" role="status"><strong>项目：</strong>{projectName ?? projectId}<span>({projectId})</span><small>所有选择均按明确 ID 保存，不从名称、路径或文本推断关系。</small></div>

      {metadataLoading && <p className="disabled-note" role="status">正在加载 Prompt、模型和工具目录…</p>}
      {metadataError && <p className="disabled-note" role="status">{metadataError}</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && !error && <p className="studio-notice" role="status">{notice}</p>}

      <div className="direct-generation-context-grid">
        <label><span>目标</span><select value={selectedShotId} onChange={(event) => setSelectedShotId(event.target.value)} disabled={busy}>
          <option value="">通用生成（无 Shot）</option>
          {shots.map((shot) => <option key={shot.id} value={shot.id}>Shot {shot.ordinal + 1} · {shot.name}</option>)}
        </select></label>
        <label><span>阶段</span><select value={stage} onChange={(event) => setStage(event.target.value as ShotStage)} disabled={busy || !selectedShotId}>
          <option value="image">图片</option><option value="video">视频</option>
        </select></label>
        <label><span>生成名称</span><input value={name} maxLength={120} onChange={(event) => setName(event.target.value)} disabled={busy} /></label>
      </div>
      {selectedShot && <p className="direct-generation-target-note" role="status">已绑定明确 Shot：{selectedShot.name} · {selectedShot.id} · {stage === "image" ? "图片阶段" : "视频阶段"}</p>}
      {!selectedShot && <p className="direct-generation-target-note" role="status">当前为项目级通用生成，不绑定 Shot。</p>}

      <div className="direct-generation-source-grid">
        <label><span>提示词版本 <small>可选</small></span><select value={promptVersionId} onChange={(event) => setPromptVersionId(event.target.value)} disabled={busy}>
          <option value="">不绑定提示词（未记录）</option>
          {prompts.flatMap((prompt) => prompt.versions.map((version) => <option key={version.id} value={version.id}>{prompt.name} · v{version.version}</option>))}
        </select></label>
        <label><span>模型版本 <small>可选</small></span><select value={modelVersionId} onChange={(event) => setModelVersionId(event.target.value)} disabled={busy}>
          <option value="">不绑定模型（未记录）</option>
          {modelVersions.map(({ model, version }) => <option key={version.id} value={version.id}>{model.name} · {version.version} · {model.provider}</option>)}
        </select></label>
        <label><span>工具实例 <small>可选</small></span><select value={toolInstanceId} onChange={(event) => { setToolInstanceId(event.target.value); setToolVersionId(""); }} disabled={busy}>
          <option value="">不绑定工具（不记录工具溯源）</option>
          {toolEntries.flatMap((entry) => entry.instances.map((instance) => <option key={instance.id} value={instance.id}>{entry.tool.name} · {instance.endpoint ?? instance.path ?? instance.id} · {instance.status}</option>))}
        </select></label>
        <label><span>工具版本 <small>需先选择实例</small></span><select value={toolVersionId} onChange={(event) => setToolVersionId(event.target.value)} disabled={busy || !toolInstanceId}>
          <option value="">不绑定工具版本</option>
          {availableToolVersions.map((version) => <option key={version.id} value={version.id}>{version.version} · {version.id}</option>)}
        </select></label>
      </div>

      <div className="direct-generation-recipe-heading"><div><span className="section-label">生成参数</span><h4>生成参数</h4></div><label><span>工作流 / 配方</span><select value={selectedRecipe ? recipeKey(selectedRecipe) : ""} onChange={(event) => {
        const nextRecipe = catalog.find((recipe) => recipeKey(recipe) === event.target.value);
        if (!nextRecipe) return;
        setSelectedRecipeKey(recipeKey(nextRecipe));
        setValues((current) => migrateGenerationValues(selectedRecipe, nextRecipe, current));
        setValidationErrors({});
      }} disabled={busy || !catalog.length}>
        {!catalog.length && <option value="">暂无可用配方</option>}
        {catalog.map((recipe) => <option key={recipeKey(recipe)} value={recipeKey(recipe)}>{recipe.name} · {recipe.mode || recipe.category}</option>)}
      </select></label></div>
      {selectedRecipe ? <DynamicFormRenderer
        recipe={selectedRecipe}
        values={values}
        validationErrors={validationErrors}
        onChange={updateValue}
        onGenerate={() => void submit()}
        projectId={projectId}
      /> : <p className="disabled-note" role="status">暂无可用工作流 / Recipe，单次生成暂不可创建。</p>}

      <div className="direct-generation-actions">
        <button type="button" onClick={() => void submit()} disabled={busy || !selectedRecipe || Boolean(createdBatch)}>{busy ? "正在加入队列…" : createdBatch ? "已加入队列" : "创建单次生成"}</button>
        {createdBatch && <button type="button" className="quiet-button" onClick={() => void onOpenProductionQueue?.(createdBatch.id)} disabled={busy || !onOpenProductionQueue}>{queueOpenFailed ? "重新打开生产队列" : "打开生产队列"}</button>}
      </div>
      {createdBatch && <div className="direct-generation-created" role="status"><strong>{createdBatch.name}</strong><span>状态：{productionStatusLabel(createdBatch.status)}</span><small>批次 ID：{createdBatch.id} · 项目项：{createdBatch.items[0]?.id ?? "—"}</small><p>已使用现有生产队列；点击“开始生产”前不会创建任务、提交 Comfy 或开始执行。</p></div>}
    </section>
  );
}

function recipeKey(recipe: RecipeViewModel | undefined): string {
  return recipe ? `${recipe.workflowVersionId}::${recipe.recipeId}` : "";
}

function productionStatusLabel(status: string): string {
  switch (status) {
    case "READY": return "待启动";
    case "RUNNING": return "运行中";
    case "PAUSED": return "已暂停";
    case "COMPLETED": return "已完成";
    default: return status;
  }
}

async function loadRegistryMetadata(projectId: string): Promise<{
  prompts: PromptEntryView[];
  modelVersions: ModelVersionOption[];
  tools: ToolRegistryEntry[];
  errors: string[];
}> {
  const [promptResult, modelResult, toolResult] = await Promise.allSettled([
    listPromptLibrary(projectId, { kind: "prompt", limit: 100 }),
    listModels(),
    listTools(),
  ]);
  const errors: string[] = [];
  const prompts = promptResult.status === "fulfilled" ? promptResult.value.items : [];
  if (promptResult.status === "rejected") errors.push("提示词");
  const models = modelResult.status === "fulfilled" ? modelResult.value : [];
  if (modelResult.status === "rejected") errors.push("模型");
  const tools = toolResult.status === "fulfilled" ? toolResult.value : [];
  if (toolResult.status === "rejected") errors.push("工具");

  const modelVersionResults = await Promise.allSettled(models.map(async (model) => (
    (await listModelVersions(model.id)).map((version) => ({ model, version }))
  )));
  const modelVersions = modelVersionResults.flatMap((result) => result.status === "fulfilled" ? result.value : []);
  if (modelVersionResults.some((result) => result.status === "rejected")) errors.push("模型版本");

  const toolResults = await Promise.allSettled(tools.map(async (tool) => {
    const [instances, versions] = await Promise.all([listToolInstances(tool.id), listToolVersions(tool.id)]);
    return { tool, instances, versions };
  }));
  const toolEntries = toolResults.flatMap((result) => result.status === "fulfilled" ? [result.value] : []);
  if (toolResults.some((result) => result.status === "rejected")) errors.push("工具实例或版本");
  return { prompts, modelVersions, tools: toolEntries, errors };
}
