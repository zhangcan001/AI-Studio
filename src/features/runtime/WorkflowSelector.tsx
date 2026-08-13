import { useMemo, useState } from "react";
import type { RecipeViewModel } from "../../types/generation";
import { workflowDisplayName } from "../../i18n/statusLabels";
import { productionRuntimeForWorkflowId } from "./productRuntimeScope";
import { sameRecipeRef, type SelectedRecipeRef } from "./workflowCapabilities";

interface Props {
  stage: "image" | "video";
  candidates: RecipeViewModel[];
  selected?: RecipeViewModel;
  recommended?: RecipeViewModel;
  selectionSource?: "manual" | "recommended" | "compatible";
  onSelect: (recipe: RecipeViewModel) => void;
  onRestoreRecommendation: () => void;
  onOpenWorkflows?: (recipe?: RecipeViewModel) => void;
  disabled?: boolean;
  title?: string;
  description?: string;
}

export function WorkflowSelector({
  stage,
  candidates,
  selected,
  recommended,
  selectionSource,
  onSelect,
  onRestoreRecommendation,
  onOpenWorkflows,
  disabled = false,
  title = "工作流",
  description = "自动推荐稳定版本，也可以手动选择已启用的工作流。",
}: Props) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const filteredCandidates = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    if (!needle) return candidates;
    return candidates.filter((recipe) => (
      `${recipe.name} ${recipe.workflowId} ${recipe.recipeId} ${recipe.recipeVersion ?? ""}`.toLocaleLowerCase().includes(needle)
    ));
  }, [candidates, search]);
  const selectedRef: SelectedRecipeRef | undefined = selected
    ? { workflowVersionId: selected.workflowVersionId, recipeId: selected.recipeId }
    : undefined;
  const isRecommended = Boolean(recommended && sameRecipeRef(selectedRef, recommended));
  const isBuiltin = selected ? productionRuntimeForWorkflowId(selected.workflowId) !== undefined : false;

  return (
    <section className="workflow-selector-panel" aria-label={`${stage === "image" ? "图片" : "视频"}工作流选择`}>
      <div className="workflow-selector-current">
        <div className="workflow-selector-title">
          <span className="section-label">{title}</span>
          <strong>{selected ? workflowDisplayName(selected.workflowId, selected.name) : "没有可用工作流"}</strong>
          {selected && (
            <span className="workflow-selector-meta">
              {selected.workflowVersionId} · Recipe {selected.recipeVersion ?? selected.recipeId}
            </span>
          )}
        </div>
        <div className="workflow-selector-status">
          <span className={selectionSource === "manual" ? "workflow-selector-badge workflow-selector-badge-manual" : "workflow-selector-badge"}>
            {selectionSource === "manual" ? "手动选择" : isRecommended ? "推荐" : "兼容"}
          </span>
          {selected && <span className="workflow-selector-origin">{isBuiltin ? "内置" : "自定义"}</span>}
          <button
            type="button"
            className="quiet-button"
            onClick={() => setOpen((current) => !current)}
            disabled={disabled || !candidates.length}
            aria-expanded={open}
          >
            {open ? "收起选择" : "更换工作流"}
          </button>
        </div>
      </div>
      <p className="workflow-selector-description">{description}</p>
      {open && (
        <div className="workflow-selector-menu">
          <div className="workflow-selector-menu-toolbar">
            {candidates.length > 10 ? (
              <label>
                <span>搜索工作流</span>
                <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="名称、workflowId 或 recipeId" autoFocus />
              </label>
            ) : <span>{candidates.length} 个正式可用 Recipe</span>}
            <div>
              <button type="button" className="quiet-button" onClick={() => { onRestoreRecommendation(); setOpen(false); }} disabled={disabled || !recommended || isRecommended}>
                恢复推荐工作流
              </button>
              {onOpenWorkflows && <button type="button" className="quiet-button" onClick={() => onOpenWorkflows(selected)}>查看工作流</button>}
            </div>
          </div>
          <div className="workflow-selector-options" role="listbox" aria-label="可用工作流">
            {filteredCandidates.map((recipe) => {
              const selectedOption = sameRecipeRef(recipe, selectedRef);
              const recommendedOption = Boolean(recommended && sameRecipeRef(recipe, recommended));
              const origin = productionRuntimeForWorkflowId(recipe.workflowId) !== undefined ? "内置" : "自定义";
              return (
                <button
                  type="button"
                  role="option"
                  aria-selected={selectedOption}
                  className={`workflow-selector-option${selectedOption ? " workflow-selector-option-selected" : ""}`}
                  key={`${recipe.workflowVersionId}:${recipe.recipeId}`}
                  onClick={() => { onSelect(recipe); setOpen(false); }}
                  disabled={disabled}
                >
                  <span className="workflow-selector-option-copy">
                    <strong>{workflowDisplayName(recipe.workflowId, recipe.name)}</strong>
                    <small>{recipe.workflowVersionId} · Recipe {recipe.recipeVersion ?? recipe.recipeId}</small>
                    <span>{recipe.outputTypes?.join(" · ") || "未声明输出"} · {recipe.mode}</span>
                  </span>
                  <span className="workflow-selector-option-tags">
                    <small>{origin}</small>
                    {recommendedOption && <small>推荐</small>}
                    {selectedOption && <small>当前</small>}
                  </span>
                </button>
              );
            })}
          </div>
          {!filteredCandidates.length && <p className="disabled-note">没有匹配的工作流。</p>}
        </div>
      )}
    </section>
  );
}
