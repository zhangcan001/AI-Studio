import { useMemo } from "react";
import type { RecipeViewModel } from "../../types/generation";
import type {
  CapabilityIssueView,
  WorkflowRegistryVersionView,
  WorkflowStagingView,
} from "../../types/workflowOnboarding";
import { formatDateTime, stagingStatusLabel, workflowDisplayName } from "../../i18n/statusLabels";
import { toUserMessage } from "../../i18n/errorMessages";
import {
  latestCatalogRecipeForWorkflowItem,
  type WorkflowWorkspaceItem,
} from "./workflowWorkspaceAdapters";
import { WorkflowRegistryRowActions } from "./WorkflowRegistryActions";

export interface WorkflowWorkspaceListProps {
  items: WorkflowWorkspaceItem[];
  staging: WorkflowStagingView[];
  catalog: RecipeViewModel[];
  projectId?: string;
  search: string;
  filter: "all" | "available" | "issues" | "archived";
  selectedVersions: string[];
  workspaceLoading: boolean;
  quickTestingId?: string;
  onSearchChange: (value: string) => void;
  onFilterChange: (value: WorkflowWorkspaceListProps["filter"]) => void;
  onCompareSelected: () => void;
  onToggleSelected: (item: WorkflowWorkspaceItem) => void;
  onToggleVersionSelection: (workflowVersionId: string) => void;
  onUseInProject: (workflowId: string, recipeId: string) => void;
  onQuickTest: (item: WorkflowWorkspaceItem) => void;
  onInspectForDeletion: (item: WorkflowWorkspaceItem) => void;
  onRestore: (item: WorkflowWorkspaceItem) => void;
  onRename: (item: WorkflowWorkspaceItem) => void;
  onReidentify: (item: WorkflowWorkspaceItem) => void;
  onRecheck: (item: WorkflowWorkspaceItem) => void;
  onDuplicateRecipe: (item: WorkflowWorkspaceItem) => void;
  onOpenParameters: (item: WorkflowWorkspaceItem) => void;
  onExport: (item: WorkflowWorkspaceItem) => void;
  onToggle: (item: WorkflowWorkspaceItem) => void;
  onPurge: (item: WorkflowWorkspaceItem) => void;
  onRepairBuiltinPackage: (item: WorkflowWorkspaceItem) => void;
  onSetCurrentVersion: (item: WorkflowWorkspaceItem, version: WorkflowRegistryVersionView) => void;
  onCleanStaging: (stagingId: string) => void;
}

export function WorkflowWorkspaceList({
  items,
  staging,
  catalog,
  projectId,
  search,
  filter,
  selectedVersions,
  workspaceLoading,
  quickTestingId,
  onSearchChange,
  onFilterChange,
  onCompareSelected,
  onToggleSelected,
  onToggleVersionSelection,
  onUseInProject,
  onQuickTest,
  onInspectForDeletion,
  onRestore,
  onRename,
  onReidentify,
  onRecheck,
  onDuplicateRecipe,
  onOpenParameters,
  onExport,
  onToggle,
  onPurge,
  onRepairBuiltinPackage,
  onSetCurrentVersion,
  onCleanStaging,
}: WorkflowWorkspaceListProps) {
  const visibleItems = useMemo(() => items.filter((item) => {
    const matchesSearch = !search.trim() || (item.name ?? item.packageName).toLowerCase().includes(search.trim().toLowerCase());
    const removed = item.libraryState === "REMOVED" || item.archived;
    const needsAction = !removed && (
      !item.enabled
      || item.packageStatus !== "VALID"
      || item.capability !== "READY"
      || item.readiness !== "READY"
      || item.diagnostics.length > 0
    );
    const matchesFilter = filter === "archived"
      ? removed
      : filter === "available"
        ? !removed && !needsAction
        : filter === "issues"
          ? !removed && needsAction
          : !removed;
    return matchesSearch && matchesFilter;
  }), [filter, items, search]);

  return (
    <>
      <div className="workflow-production-toolbar">
        <input aria-label="搜索工作流" placeholder="按名称搜索" value={search} onChange={(event) => onSearchChange(event.target.value)} />
        <select aria-label="工作流筛选" value={filter} onChange={(event) => onFilterChange(event.target.value as WorkflowWorkspaceListProps["filter"])}>
          <option value="all">全部</option><option value="available">可用</option><option value="issues">需处理</option><option value="archived">已删除</option>
        </select>
        <button type="button" onClick={onCompareSelected} disabled={selectedVersions.length !== 2}>比较选中的版本</button>
      </div>
      <div className="workflow-catalog" aria-label="工作流运行包">
        <div className="workflow-catalog-header">
          <span>比较</span><span>工作流</span><span>当前版本</span><span>当前 Recipe</span><span>来源</span><span>可用性</span><span>运行记录</span><span>操作</span>
        </div>
        {workspaceLoading && !visibleItems.length && <p className="loading-state">正在读取已注册工作流…</p>}
        {visibleItems.map((item) => {
          const removed = item.libraryState === "REMOVED" || item.archived;
          const currentVersionId = item.currentVersionId;
          const currentRecipe = item.currentRecipe;
          const versionsForDisplay = item.versions;
          const recipesForDisplay = item.registryRecipes;
          const projectRecipe = latestCatalogRecipeForWorkflowItem(item, catalog);
          return (
            <article className="workflow-catalog-row" key={item.workflowId}>
              <input type="checkbox" aria-label={`比较 ${workflowDisplayName(item.workflowId, item.name ?? item.packageName)}`} checked={currentVersionId ? selectedVersions.includes(currentVersionId) : false} onChange={() => onToggleSelected(item)} disabled={!currentVersionId} />
              <div className="workflow-row-identity">
                <strong>{workflowDisplayName(item.workflowId, item.name ?? item.packageName)}</strong>
                <small>{workflowSourceLabel(item)} · {item.versions.length} 个版本</small>
              </div>
              <span>{item.workflowVersion ?? "—"}{removed ? " · 已删除" : ""}</span>
              <span>{currentRecipe?.version ?? "—"} · {item.recipes.length} 个配方</span>
              <span>{workflowSourceLabel(item)}</span>
              <span className={`workflow-capability workflow-capability-${item.capability.toLowerCase()}`}>{formatCapability(item.capability)}</span>
              <span>{item.hasSuccessfulRun ? "已有成功运行" : `共 ${item.totalTasks} 个任务`}</span>
              <WorkflowRegistryRowActions
                item={item}
                projectId={projectId}
                projectRecipe={projectRecipe}
                currentVersionId={currentVersionId}
                quickTesting={quickTestingId === currentVersionId}
                onUseInProject={onUseInProject}
                onQuickTest={() => onQuickTest(item)}
                onInspectForDeletion={() => onInspectForDeletion(item)}
                onRestore={() => onRestore(item)}
                onRename={() => onRename(item)}
                onReidentify={() => onReidentify(item)}
                onRecheck={() => onRecheck(item)}
                onDuplicateRecipe={() => onDuplicateRecipe(item)}
                onOpenParameters={() => onOpenParameters(item)}
                onExport={() => onExport(item)}
                onToggle={() => onToggle(item)}
                onPurge={() => onPurge(item)}
              />
              <details className="workflow-catalog-detail">
                <summary>查看详情</summary>
                <div className="workflow-detail-grid">
                  <span>启用状态 <strong>{removed ? "已删除" : item.enabled ? "已启用" : "已停用"}</strong></span>
                  <span>工作流状态 <strong>{removed ? "已删除" : "活跃"}</strong></span>
                  <span>来源 <strong>{workflowSourceLabel(item)}</strong></span>
                  <span>工作流 SHA-256 <strong>{item.workflowSha256 || "—"}</strong></span>
                  <span>配方 SHA-256 <strong>{item.recipeSha256 ?? "—"}</strong></span>
                  <span>节点数量 <strong>{item.nodeCount}</strong></span>
                  <span>当前版本 <strong>{item.workflowVersion ?? "—"}</strong></span>
                  <span>当前 Recipe <strong>{currentRecipe?.version ?? "—"}</strong></span>
                  <span>项目使用数 <strong>{item.projectUsageCount}</strong></span>
                  <span>历史记录数 <strong>{item.historyCount}</strong></span>
                  <span>活动任务 <strong>{item.activeTasks}</strong></span>
                  <span>任务总数 <strong>{item.totalTasks}</strong></span>
                  <span>最近真实验证 <strong>{item.liveVerifiedAt ? formatDateTime(item.liveVerifiedAt) : "尚未验证"}</strong></span>
                </div>
                {!!item.readinessReasons.length && <ul className="workflow-issue-list">{item.readinessReasons.map((reason) => <li key={reason}>{reason}</li>)}</ul>}
                {!!item.capabilityIssues.length && <section className="workflow-dependency-diagnostics"><strong>依赖诊断 · 来源：ComfyUI /object_info</strong><IssueList issues={item.capabilityIssues} /></section>}
                {!item.capabilityIssues.length && item.packageStatus === "VALID" && <p className="disabled-note">未发现节点依赖问题。模型或文件依赖只有在运行包明确声明并有可验证来源时才会报告，AI Studio 不猜测未声明依赖。</p>}
                {!!item.diagnostics.length && <ul className="workflow-issue-list">{item.diagnostics.map((diagnostic) => <li key={diagnostic.code}>{toUserMessage({ code: diagnostic.code, message: diagnostic.message })}</li>)}</ul>}
                {item.diagnostics.some((diagnostic) => diagnostic.code === "BUILTIN_PACKAGE_HASH_MISMATCH") && <button type="button" className="quiet-button danger-button" onClick={() => onRepairBuiltinPackage(item)}>修复内置包哈希</button>}
                <section className="workflow-registry-nested" aria-label="工作流版本">
                  <h4>Versions</h4>
                  {versionsForDisplay.map((version) => (
                    <div className="workflow-registry-version" key={version.workflowVersionId}>
                      <input type="checkbox" aria-label={`比较版本 ${version.version ?? version.workflowVersion ?? "—"}`} checked={selectedVersions.includes(version.workflowVersionId)} onChange={() => onToggleVersionSelection(version.workflowVersionId)} />
                      <strong>{version.version ?? version.workflowVersion ?? "—"}{version.workflowVersionId === currentVersionId ? " · 当前" : ""}</strong>
                      <span>{(version.recipes ?? []).length} 个 Recipe</span>
                      {item.registryBacked && !removed && version.workflowVersionId !== currentVersionId && <button type="button" className="quiet-button" onClick={() => onSetCurrentVersion(item, version)}>设为当前版本</button>}
                    </div>
                  ))}
                </section>
                <section className="workflow-registry-nested" aria-label="工作流 Recipe">
                  <h4>Recipes</h4>
                  <div className="workflow-recipe-summary">
                    {recipesForDisplay.map((recipe) => <span key={`${recipe.workflowVersionId ?? currentVersionId ?? "version"}:${recipe.recipeId}`}>配方 {recipe.version ?? recipe.recipeVersion ?? "—"} · {recipe.inputCount ?? 0} 个输入 · {recipe.outputCount ?? 0} 个输出</span>)}
                  </div>
                </section>
              </details>
            </article>
          );
        })}
        {!visibleItems.length && !workspaceLoading && <p className="empty-state">当前筛选条件下没有找到工作流。</p>}
      </div>

      {!!staging.length && <div className="workflow-diagnostics-panel"><h3>运行诊断</h3>{staging.map((entry) => <div key={entry.stagingId}><span>{stagingStatusLabel(entry.status)}</span><code>{entry.stagingId}</code><button type="button" className="quiet-button" onClick={() => onCleanStaging(entry.stagingId)} disabled={entry.inUse}>{entry.inUse ? "使用中" : "清理暂存"}</button></div>)}</div>}
    </>
  );
}

function IssueList({ issues }: { issues: CapabilityIssueView[] }) {
  return <ul className="workflow-issue-list">{issues.map((issue, index) => <li key={`${issue.code}-${issue.nodeId ?? ""}-${index}`}>{issue.message}</li>)}</ul>;
}

function workflowSourceLabel(item: WorkflowWorkspaceItem): string {
  return item.sourceKind === "PRODUCT" ? "内置工作流" : "用户工作流";
}

function formatCapability(value: string): string {
  const normalized = value.trim().toUpperCase();
  if (normalized === "READY") return "就绪";
  if (normalized === "MISSING_NODES") return "缺少节点";
  if (normalized === "COMFY_OFFLINE" || normalized === "OFFLINE") return "ComfyUI 离线";
  if (normalized.includes("INCOMPATIBLE")) return "参数不兼容";
  if (normalized === "NOT_CHECKED") return "待检查";
  return value || "未知";
}
