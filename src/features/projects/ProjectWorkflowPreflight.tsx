import { useEffect, useMemo, useState } from "react";
import { getProjectWorkflowConfig } from "../../services/tauriClient";
import type { RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { toUserMessage } from "../../i18n/errorMessages";
import { workflowDisplayName } from "../../i18n/statusLabels";
import {
  preflightProjectWorkflow,
  type ProjectWorkflowOverallStatus,
  type ProjectWorkflowPreflightItem,
  type ProjectWorkflowPreflightSource,
} from "../runtime/projectWorkflowPreflight";

interface Props {
  config: ProjectWorkflowConfigView;
  catalog: RecipeViewModel[];
}

const PATH_LABELS: Record<ProjectWorkflowPreflightItem["path"], string> = {
  IMAGE: "图片生成",
  FL2VA_TEXT_TO_VIDEO: "文生视频",
  FL2VA_IMAGE_TO_VIDEO: "图生视频",
  FL2VA_FIRST_LAST: "首尾帧视频",
  REF2VA_IMAGE: "参考图视频",
  REF2VA_AUDIO: "参考音频视频",
  REF2VA_IMAGE_AUDIO: "参考图 + 音频",
  REF2VA_VIDEO_IMAGE: "参考视频 + 参考图",
};

const STATUS_LABELS: Record<ProjectWorkflowPreflightItem["status"], string> = {
  READY: "可生产",
  WARNING: "需要注意",
  BLOCKED: "不可用",
};

function sourceLabel(path: ProjectWorkflowPreflightItem["path"], source: ProjectWorkflowPreflightSource | undefined): string {
  if (source === "project_mode") return "模式专用";
  if (source === "project_default") return path === "IMAGE" ? "项目图片默认" : "项目视频默认";
  if (source === "recommended") return "系统推荐";
  if (source === "compatible") return "兼容回退";
  return "—";
}

function overallLabel(status: ProjectWorkflowOverallStatus): string {
  if (status === "READY") return "✓ 项目工作流可生产";
  if (status === "PARTIAL") return "⚠ 项目工作流部分可用";
  return "✕ 当前没有可用生产工作流";
}

function recipeLabel(recipe: RecipeViewModel): string {
  return workflowDisplayName(recipe.workflowId, recipe.name);
}

function PreflightItem({ item }: { item: ProjectWorkflowPreflightItem }) {
  const stale = item.staleConfiguredBinding;
  return (
    <article
      className={`project-workflow-preflight-item project-workflow-preflight-item-${item.status.toLowerCase()}`}
      data-testid="project-workflow-preflight-item"
    >
      <div className="project-workflow-preflight-item-heading">
        <strong>{PATH_LABELS[item.path]}</strong>
        <span className="project-workflow-preflight-status" aria-label={STATUS_LABELS[item.status]}>
          {item.status === "READY" ? "✓" : item.status === "WARNING" ? "⚠" : "✕"} {STATUS_LABELS[item.status]}
        </span>
      </div>
      {item.recipe ? (
        <>
          <div className="project-workflow-preflight-recipe"><strong>使用：{recipeLabel(item.recipe)}</strong></div>
          <small>WorkflowVersion：{item.recipe.workflowVersionId} · Recipe：{item.recipe.recipeId}</small>
          <small>来源：{sourceLabel(item.path, item.source)}</small>
          {stale && (
            <p className="project-workflow-preflight-warning" role="alert">
              ⚠ 项目绑定不可用，当前实际使用：{recipeLabel(item.recipe)}。原 WorkflowVersion：{item.configuredRef?.workflowVersionId} · 原 Recipe：{item.configuredRef?.recipeId}。建议重新选择或清除失效绑定。
            </p>
          )}
        </>
      ) : (
        <>
          <p className="project-workflow-preflight-blocked">✕ 当前无兼容工作流</p>
          {stale && (
            <p className="project-workflow-preflight-warning" role="alert">
              ⚠ 项目绑定不可用。原 WorkflowVersion：{item.configuredRef?.workflowVersionId} · 原 Recipe：{item.configuredRef?.recipeId}。请重新选择或清除失效绑定。
            </p>
          )}
        </>
      )}
    </article>
  );
}

export function ProjectWorkflowPreflight({ config, catalog }: Props) {
  const [currentConfig, setCurrentConfig] = useState(config);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    setCurrentConfig(config);
    setError(undefined);
  }, [config]);

  const report = useMemo(
    () => preflightProjectWorkflow(currentConfig, catalog),
    [catalog, currentConfig],
  );

  async function refresh() {
    setRefreshing(true);
    setError(undefined);
    try {
      setCurrentConfig(await getProjectWorkflowConfig(currentConfig.projectId));
    } catch (value) {
      setError(toUserMessage(value));
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <section className="project-workflow-preflight" aria-labelledby="project-workflow-preflight-title">
      <div className="section-heading">
        <div>
          <span className="section-label">项目工作流</span>
          <h3 id="project-workflow-preflight-title">生产可用性</h3>
          <p className="section-description">根据当前工作流目录、Recipe 能力和项目绑定实时计算；结果不会写入数据库。</p>
        </div>
        <button type="button" className="quiet-button" onClick={() => void refresh()} disabled={refreshing}>
          {refreshing ? "正在检查…" : "重新检查"}
        </button>
      </div>
      {error && <p className="error-message" role="alert">重新检查失败：{error}</p>}
      <div className={`project-workflow-preflight-summary project-workflow-preflight-overall-${report.overallStatus.toLowerCase()}`} role="status" aria-live="polite">
        <strong>{overallLabel(report.overallStatus)}</strong>
        <span>{report.readyCount} / {report.totalCount} 条生成路径可用</span>
        {report.warningCount > 0 && <span>⚠ {report.warningCount} 项需要注意：存在失效项目绑定，当前正在使用 fallback。</span>}
        {report.overallStatus === "PARTIAL" && <span>{report.blockedCount} 条需要配置兼容工作流</span>}
      </div>
      <div className="project-workflow-preflight-group">
        <h4>图片生成</h4>
        <PreflightItem item={report.items[0]} />
      </div>
      <div className="project-workflow-preflight-group">
        <h4>视频生成</h4>
        <div className="project-workflow-preflight-grid">
          {report.items.slice(1).map((item) => <PreflightItem key={item.path} item={item} />)}
        </div>
      </div>
    </section>
  );
}
