import { useState } from "react";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import { defaultGenerationValues } from "../../stores/studioStore";
import { DynamicFormRenderer, validateRecipeValues } from "../studio/DynamicFormRenderer";
import { WorkflowBenchmarkPanel } from "../experiments/WorkflowBenchmarkPanel";
/** Local validation form only; Benchmark service/Queue remains the authority. */
export function LabBenchmarkPane({ projectId, catalog, onOpenTask }: { projectId: string; catalog: RecipeViewModel[]; onOpenTask?: (id: string) => void }) {
  const [key, setKey] = useState("");
  const [values, setValues] = useState<GenerationValues>({});
  const recipe = catalog.find(r => JSON.stringify([r.workflowVersionId, r.recipeId]) === key);
  const errors = recipe ? validateRecipeValues(recipe, values) : {};
  return <section aria-label="高级验证 Benchmark"><h2>高级验证 · Benchmark</h2><p>显式选择技术版本比较结果；不会自动选为镜头结果、推广配方或修改项目绑定。</p>
    <label>基准配方<select value={key} onChange={e => { const next = catalog.find(r => JSON.stringify([r.workflowVersionId, r.recipeId]) === e.target.value); setKey(e.target.value); setValues(next ? defaultGenerationValues(next) : {}); }}><option value="">选择确切版本和配方</option>{catalog.map(r => <option key={JSON.stringify([r.workflowVersionId, r.recipeId])} value={JSON.stringify([r.workflowVersionId, r.recipeId])}>{r.name} · {r.workflowVersionId} · {r.recipeId}</option>)}</select></label>
    {recipe && <DynamicFormRenderer recipe={recipe} values={values} validationErrors={errors} projectId={projectId} onGenerate={() => undefined} onChange={(field, value) => setValues(v => { const next = { ...v }; if (value) next[field] = value; else delete next[field]; return next; })} />}
    <WorkflowBenchmarkPanel projectId={projectId} catalog={catalog} baseRecipe={recipe} baseValues={values} baseReady={!!recipe && Object.keys(errors).length === 0} blockedReason={!recipe ? "请选择基准配方。" : Object.keys(errors).length ? "请补充必需输入。" : undefined} onOpenTask={onOpenTask} />
  </section>;
}
