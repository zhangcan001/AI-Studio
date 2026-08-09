import type { RecipeViewModel } from "../../types/generation";
import { workflowDescription, workflowDisplayName, workflowModeLabel } from "../../i18n/statusLabels";

interface Props {
  catalog: RecipeViewModel[];
  selectedWorkflow?: RecipeViewModel;
  onSelect: (workflow: RecipeViewModel) => void;
}

export function WorkflowLauncher({ catalog, selectedWorkflow, onSelect }: Props) {
  return (
    <section className="workflow-launcher" aria-labelledby="workflow-launcher-title">
      <div className="section-heading workflow-launcher-heading">
        <div>
          <span className="section-label">快速开始</span>
          <h2 id="workflow-launcher-title">选择创作类型</h2>
          <p className="section-description">先选一个工作流，再填写输入参数。</p>
        </div>
        <span className="workflow-launcher-count">{catalog.length} 个工作流</span>
      </div>
      <div className="workflow-card-grid" role="list" aria-label="可用工作流">
        {catalog.map((recipe) => {
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
                  <small>{workflowModeLabel(recipe.mode)}</small>
                  <span>{workflowDescription(recipe.mode)}</span>
                </span>
                <span className="workflow-card-state" aria-hidden="true">{selected ? "已选择" : "选择"}</span>
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}
