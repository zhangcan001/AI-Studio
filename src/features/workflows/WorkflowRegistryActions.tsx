import type { RecipeViewModel } from "../../types/generation";
import type { WorkflowRegistryVersionView } from "../../types/workflowOnboarding";
import { workflowDisplayName } from "../../i18n/statusLabels";
import { type WorkflowWorkspaceItem, canPurgeWorkflow } from "./workflowWorkspaceAdapters";

export interface WorkflowRegistryActionsProps {
  loading: boolean;
  workspaceLoading: boolean;
  checkingAll: boolean;
  importBusy: boolean;
  onRefresh: () => void;
  onAdd: () => void;
  onCheckAll: () => void;
  onManualImport: () => void;
  onImportBackup: () => void;
}

/** Top-level Registry/Lifecycle controls. */
export function WorkflowRegistryActions({
  loading,
  workspaceLoading,
  checkingAll,
  importBusy,
  onRefresh,
  onAdd,
  onCheckAll,
  onManualImport,
  onImportBackup,
}: WorkflowRegistryActionsProps) {
  return (
    <div className="workflow-workspace-actions">
      <button type="button" onClick={onRefresh} disabled={workspaceLoading}>{workspaceLoading ? "正在刷新..." : "刷新"}</button>
      <button type="button" onClick={onAdd} disabled={loading || importBusy}>+ 添加工作流</button>
      <details className="workflow-advanced-actions">
        <summary>更多</summary>
        <div className="workflow-advanced-actions-content">
          <button type="button" className="quiet-button" onClick={onCheckAll} disabled={checkingAll || workspaceLoading}>{checkingAll ? "检查中..." : "检查全部兼容性"}</button>
          <button type="button" className="quiet-button" onClick={onManualImport} disabled={loading || importBusy}>手动配置工作流</button>
          <button type="button" className="quiet-button" onClick={onImportBackup} disabled={loading || importBusy}>导入工作流备份</button>
        </div>
      </details>
    </div>
  );
}

export interface WorkflowRegistryRowActionsProps {
  item: WorkflowWorkspaceItem;
  projectId?: string;
  projectRecipe?: RecipeViewModel;
  currentVersionId?: string;
  quickTesting: boolean;
  onUseInProject: (workflowId: string, recipeId: string) => void;
  onQuickTest: () => void;
  onInspectForDeletion: () => void;
  onRestore: () => void;
  onRename: () => void;
  onReidentify: () => void;
  onRecheck: () => void;
  onDuplicateRecipe: () => void;
  onOpenParameters: () => void;
  onExport: () => void;
  onToggle: () => void;
  onPurge: () => void;
}

/** Row actions stay presentational; lifecycle state changes remain in WorkflowWorkspace. */
export function WorkflowRegistryRowActions({
  item,
  projectId,
  projectRecipe,
  currentVersionId,
  quickTesting,
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
}: WorkflowRegistryRowActionsProps) {
  const removed = item.libraryState === "REMOVED" || item.archived;
  const canPurge = item.registryBacked && canPurgeWorkflow(item);
  return (
    <div className="workflow-row-actions">
      {!removed && currentVersionId && (() => (
        <button
          type="button"
          className="quiet-button"
          onClick={() => { if (projectRecipe && item.workflowId) onUseInProject(item.workflowId, projectRecipe.recipeId); }}
          disabled={!projectId || !projectRecipe}
          title={!projectId ? "请先选择或创建一个项目" : !projectRecipe ? "该工作流尚未进入生产目录" : "在当前项目中配置这个工作流"}
        >用于当前项目</button>
      ))()}
      {currentVersionId && !removed && <button type="button" className="quiet-button" onClick={onQuickTest} disabled={quickTesting}>{quickTesting ? "测试中..." : "测试"}</button>}
      {!removed && <button type="button" className="quiet-button danger-button" onClick={onInspectForDeletion} title="从工作流库移除这个工作流">删除</button>}
      {removed && <button type="button" className="quiet-button" onClick={onRestore}>恢复工作流</button>}
      <details className="workflow-row-menu">
        <summary aria-label="更多工作流操作">⋯</summary>
        <div className="workflow-row-menu-content">
          {!removed && item.registryBacked && <button type="button" className="quiet-button" onClick={onRename}>重命名</button>}
          {!removed && currentVersionId && item.workflowId && item.workflowVersion && <button type="button" className="quiet-button" onClick={onReidentify}>重新识别</button>}
          {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={onRecheck}>重新检查</button>}
          {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={onDuplicateRecipe}>创建新 Recipe 版本</button>}
          {!removed && currentVersionId && <button type="button" className="quiet-button workflow-parameter-button" onClick={onOpenParameters}>生产参数</button>}
          {currentVersionId && <button type="button" className="quiet-button" onClick={onExport}>导出工作流</button>}
          {!removed && currentVersionId && <button type="button" className="quiet-button" onClick={onToggle}>{item.enabled ? "停用" : "启用"}</button>}
          {canPurge && <button type="button" className="quiet-button danger-button" onClick={onPurge}>彻底删除</button>}
        </div>
      </details>
    </div>
  );
}

export function workspaceRowLabel(item: WorkflowWorkspaceItem): string {
  return workflowDisplayName(item.workflowId, item.name ?? item.packageName);
}

export function versionIsCurrent(version: WorkflowRegistryVersionView, currentVersionId?: string): boolean {
  return version.workflowVersionId === currentVersionId;
}
