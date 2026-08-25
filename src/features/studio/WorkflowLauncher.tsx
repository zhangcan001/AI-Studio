import { useMemo, useState } from "react";
import type { RecipeViewModel } from "../../types/generation";
import { workflowDescription, workflowDisplayName, workflowModeLabel } from "../../i18n/statusLabels";
import { filterRuntimeCatalog, runtimeKindFor, runtimeKindLabel, type RuntimeFilter } from "../runtime/pack05";
import { filterProductionRuntimeCatalog } from "../runtime/productRuntimeScope";

interface Props {
  catalog: RecipeViewModel[];
  selectedWorkflow?: RecipeViewModel;
  onSelect: (workflow: RecipeViewModel) => void;
}

export function WorkflowLauncher({ catalog, selectedWorkflow, onSelect }: Props) {
  const [filter, setFilter] = useState<RuntimeFilter>("all");
  const [search, setSearch] = useState("");
  const productCatalog = useMemo(() => filterProductionRuntimeCatalog(catalog), [catalog]);
  const visibleCatalog = useMemo(() => filterRuntimeCatalog(productCatalog, filter, search), [productCatalog, filter, search]);

  return (
    <section className="workflow-launcher" aria-labelledby="workflow-launcher-title">
      <div className="section-heading workflow-launcher-heading">
        <div>
          <span className="section-label">快速开始</span>
          <h2 id="workflow-launcher-title">选择创作类型</h2>
          <p className="section-description">先选一个工作流，再填写输入参数。</p>
        </div>
        <span className="workflow-launcher-count">{visibleCatalog.length} / {productCatalog.length} 个配方</span>
      </div>
      <div className="workflow-launcher-controls">
        <label>
          <span>创作类型</span>
          <select aria-label="运行时类型筛选" value={filter} onChange={(event) => setFilter(event.target.value as RuntimeFilter)}>
            <option value="all">全部</option>
            <option value="image">图片</option>
            <option value="video">视频</option>
            <option value="audio">音频</option>
            <option value="mixed">复合</option>
          </select>
        </label>
        <label>
          <span>搜索</span>
          <input aria-label="搜索运行时" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="按名称或模式搜索" />
        </label>
      </div>
      <div className="workflow-card-grid" role="list" aria-label="可用工作流">
        {visibleCatalog.map((recipe) => {
          const selected = recipe.workflowVersionId === selectedWorkflow?.workflowVersionId && recipe.recipeId === selectedWorkflow?.recipeId;
          return (
            <div key={`${recipe.workflowVersionId}:${recipe.recipeId}`} role="listitem">
              <button
                type="button"
                className={`workflow-launcher-card${selected ? " workflow-launcher-card-selected" : ""}`}
                aria-pressed={selected}
                onClick={() => onSelect(recipe)}
              >
                <span className="workflow-card-mark" aria-hidden="true">{workflowModeLabel(recipe.mode).slice(0, 1)}</span>
                <span className="workflow-card-copy">
                  <strong>{workflowDisplayName(recipe.workflowId, recipe.name)}</strong>
                  <small>{runtimeKindLabel(runtimeKindFor(recipe))} · {workflowModeLabel(recipe.mode)} · 配方 {recipe.recipeVersion ?? recipe.recipeId}</small>
                  <span>{workflowDescription(recipe.mode)}</span>
                </span>
                <span className="workflow-card-state" aria-hidden="true">{selected ? "已选择" : "选择"}</span>
              </button>
            </div>
          );
        })}
      </div>
      {!visibleCatalog.length && <p className="disabled-note">没有匹配的运行时，请调整筛选条件。</p>}
    </section>
  );
}
