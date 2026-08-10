import { useMemo, useState } from "react";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import {
  buildExperimentPlan,
  experimentVariantFields,
  freezeSeedVariants,
  removeExperimentPlanItem,
  type ExperimentDimension,
  type ExperimentPlan,
  type SeedFieldDefinition,
} from "./experimentPlanner";

interface Props {
  recipe: RecipeViewModel;
  baseValues: GenerationValues;
  baseReady: boolean;
  blockedReason?: string;
  onSubmit: (plan: ExperimentPlan) => Promise<void>;
}

interface DimensionDraft {
  id: string;
  fieldKey: string;
  seedMode: "fixed" | "random";
  rawValues: string;
  randomCount: string;
}

function newDimension(id: string, fieldKey: string): DimensionDraft {
  return { id, fieldKey, seedMode: "fixed", rawValues: "", randomCount: "2" };
}

export function ExperimentPlannerPanel({ recipe, baseValues, baseReady, blockedReason, onSubmit }: Props) {
  const fields = useMemo(() => experimentVariantFields(recipe), [recipe]);
  const fieldMap = useMemo(() => new Map(fields.map((field) => [field.key, field])), [fields]);
  const [dimensions, setDimensions] = useState<DimensionDraft[]>([]);
  const [plan, setPlan] = useState<ExperimentPlan>();
  const [issues, setIssues] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);

  function changeDimension(id: string, patch: Partial<DimensionDraft>) {
    setDimensions((current) => current.map((dimension) => dimension.id === id ? { ...dimension, ...patch } : dimension));
    setPlan(undefined);
    setIssues([]);
  }

  function addDimension() {
    if (dimensions.length >= 2) return;
    const nextField = fields.find((field) => !dimensions.some((dimension) => dimension.fieldKey === field.key));
    if (!nextField) {
      setIssues(["当前 Recipe 没有更多可实验的文字、整数或 Seed 字段。"]);
      return;
    }
    setDimensions((current) => [...current, newDimension(`dimension-${Date.now()}-${current.length}`, nextField.key)]);
    setPlan(undefined);
    setIssues([]);
  }

  function removeDimension(id: string) {
    setDimensions((current) => current.filter((dimension) => dimension.id !== id));
    setPlan(undefined);
    setIssues([]);
  }

  function parseDimensions(): { dimensions: ExperimentDimension[]; issues: string[] } {
    const nextDimensions: ExperimentDimension[] = [];
    const nextIssues: string[] = [];
    for (const draft of dimensions) {
      const field = fieldMap.get(draft.fieldKey);
      if (!field) {
        nextIssues.push("实验字段已失效，请重新选择。 ");
        continue;
      }
      if (field.type === "textarea") {
        const values = draft.rawValues.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
        nextDimensions.push({ fieldKey: field.key, values: values.map((value) => ({ type: "string", value })) });
      } else if (field.type === "integer") {
        const rawValues = draft.rawValues.split(/[\s,，]+/).map((value) => value.trim()).filter(Boolean);
        const invalid = rawValues.filter((value) => !Number.isSafeInteger(Number(value)));
        if (invalid.length) nextIssues.push(`字段“${field.label}”包含无效整数：${invalid.join("、")}。`);
        const values = rawValues
          .map((value) => Number(value))
          .filter((value) => Number.isSafeInteger(value))
          .map((value) => ({ type: "integer" as const, value }));
        nextDimensions.push({ fieldKey: field.key, values });
      } else if (field.type === "seed") {
        if (draft.seedMode === "random") {
          const frozen = freezeSeedVariants(field as SeedFieldDefinition, Number(draft.randomCount));
          nextIssues.push(...frozen.issues);
          nextDimensions.push({ fieldKey: field.key, values: frozen.values });
        } else {
          const values = draft.rawValues.split(/[\s,，]+/).map((value) => value.trim()).filter(Boolean).map((value) => ({ type: "seed_fixed" as const, value }));
          nextDimensions.push({ fieldKey: field.key, values });
        }
      }
    }
    return { dimensions: nextDimensions, issues: nextIssues };
  }

  function previewPlan() {
    const parsed = parseDimensions();
    const result = buildExperimentPlan({ recipe, baseValues, dimensions: parsed.dimensions });
    const nextIssues = [...parsed.issues, ...result.issues];
    setIssues([...new Set(nextIssues)]);
    setPlan(result.plan);
  }

  async function submitPlan() {
    if (!plan || !plan.items.length || !baseReady || submitting) return;
    if (plan.items.length > 8 && !window.confirm(`本次实验包含 ${plan.items.length} 个生成任务，将按顺序执行，是否继续？`)) return;
    setSubmitting(true);
    try {
      await onSubmit(plan);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="experiment-planner" aria-label="实验计划">
      <div className="experiment-planner-heading">
        <div>
          <span className="section-label">实验计划</span>
          <h3>从当前创作生成参数变体</h3>
          <p>只组合当前 Recipe 的文字、整数和 Seed 字段；图片、视频、音频素材继续复用基础 Draft。</p>
        </div>
        <span className="experiment-dimension-count">{dimensions.length} / 2 个维度</span>
      </div>

      <div className="experiment-dimension-list">
        {dimensions.map((dimension, index) => {
          const field = fieldMap.get(dimension.fieldKey);
          return (
            <article className="experiment-dimension-card" key={dimension.id}>
              <div className="experiment-dimension-heading">
                <strong>维度 {index + 1}</strong>
                <button type="button" className="quiet-button" onClick={() => removeDimension(dimension.id)}>删除</button>
              </div>
              <div className="experiment-dimension-form">
                <label>
                  <span>变化字段</span>
                  <select aria-label={`实验维度 ${index + 1} 字段`} value={dimension.fieldKey} onChange={(event) => changeDimension(dimension.id, { fieldKey: event.target.value, rawValues: "", seedMode: "fixed" })}>
                    {fields.map((candidate) => <option key={candidate.key} value={candidate.key} disabled={dimensions.some((other) => other.id !== dimension.id && other.fieldKey === candidate.key)}>{candidate.label} · {candidate.type}</option>)}
                  </select>
                </label>
                {field?.type === "seed" && (
                  <label>
                    <span>Seed 方式</span>
                    <select value={dimension.seedMode} onChange={(event) => changeDimension(dimension.id, { seedMode: event.target.value as DimensionDraft["seedMode"], rawValues: "" })}>
                      <option value="fixed">固定 Seed 列表</option>
                      <option value="random">冻结随机 Seed</option>
                    </select>
                  </label>
                )}
                {field?.type === "seed" && dimension.seedMode === "random" ? (
                  <label>
                    <span>随机数量</span>
                    <input type="number" min={1} max={24} step={1} value={dimension.randomCount} onChange={(event) => changeDimension(dimension.id, { randomCount: event.target.value })} />
                  </label>
                ) : (
                  <label className="experiment-values-input">
                    <span>{field?.type === "textarea" ? "文本变体（每行一个，最多 8 个）" : field?.type === "integer" ? `整数变体（空格或逗号分隔${field.min !== undefined && field.max !== undefined ? `，范围 ${field.min}–${field.max}` : ""}）` : "固定 Seed 列表（空格或逗号分隔）"}</span>
                    <textarea rows={field?.type === "textarea" ? 4 : 2} value={dimension.rawValues} onChange={(event) => changeDimension(dimension.id, { rawValues: event.target.value })} placeholder={field?.type === "textarea" ? "版本 A\n版本 B" : "例如：8 12 16"} />
                  </label>
                )}
              </div>
            </article>
          );
        })}
      </div>

      <div className="experiment-planner-actions">
        <button type="button" className="quiet-button" onClick={addDimension} disabled={dimensions.length >= 2}>添加变体维度</button>
        <button type="button" onClick={previewPlan} disabled={!dimensions.length}>预览实验计划</button>
      </div>

      {!baseReady && <p className="experiment-blocked-reason" role="status">请先完成基础 Draft：{blockedReason ?? "输入校验或素材检查尚未通过。"}</p>}
      {issues.length > 0 && <ul className="experiment-issue-list">{issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
      {plan && (
        <>
          <div className="experiment-plan-summary">
            <strong>本次实验将生成 {plan.items.length} 个任务。</strong>
            <span>计划冻结时间：{new Date(plan.frozenAt).toLocaleString()}</span>
            {plan.videoWarning && <span className="experiment-video-warning">视频实验可能耗时较长，任务仍会严格顺序执行。</span>}
          </div>
          <div className="experiment-plan-table" role="table" aria-label="实验计划预览">
            <div className="experiment-plan-row experiment-plan-header" role="row"><span>序号</span><span>变化字段</span><span>Seed</span><span>预计任务数量</span><span>操作</span></div>
            {plan.items.map((item) => (
              <div className="experiment-plan-row" role="row" key={item.id}>
                <span>#{item.ordinal + 1}</span>
                <span>{item.changes.map((change) => `${change.fieldLabel}：${change.value}`).join(" · ")}</span>
                <span>{item.seed ?? "—"}</span>
                <span>1</span>
                <button type="button" className="quiet-button" onClick={() => setPlan(removeExperimentPlanItem(plan, item.id))}>删除</button>
              </div>
            ))}
          </div>
          <button type="button" className="experiment-submit-button" onClick={() => void submitPlan()} disabled={!plan.items.length || !baseReady || submitting}>
            {submitting ? "正在加入实验队列..." : "加入实验队列"}
          </button>
        </>
      )}
    </section>
  );
}
