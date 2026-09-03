import { useEffect, useMemo, useRef, useState } from "react";
import { getComfyPreflight } from "../../services/tauriClient";
import type { RecipeViewModel } from "../../types/generation";
import type {
  ComfyPreflightIssue,
  ComfyPreflightReport,
  ComfyPreflightWorkflowItem,
} from "../../types/settings";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, formatFileSize, workflowDisplayName } from "../../i18n/statusLabels";
import {
  composeProjectProductionReadiness,
  type ProjectProductionPathReadiness,
  type ProjectProductionReadinessReport,
  type ProjectProductionReadinessStatus,
} from "../runtime/projectProductionReadiness";
import {
  preflightProjectWorkflow,
  type ProjectWorkflowPreflightSource,
} from "../runtime/projectWorkflowPreflight";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";

interface Props {
  config: ProjectWorkflowConfigView;
  catalog: RecipeViewModel[];
}

const PATH_LABELS: Record<ProjectProductionPathReadiness["path"], string> = {
  IMAGE: "图片生成",
  FL2VA_TEXT_TO_VIDEO: "文生视频",
  FL2VA_IMAGE_TO_VIDEO: "图生视频",
  FL2VA_FIRST_LAST: "首尾帧视频",
  REF2VA_IMAGE: "参考图视频",
  REF2VA_AUDIO: "参考音频视频",
  REF2VA_IMAGE_AUDIO: "参考图 + 音频",
  REF2VA_VIDEO_IMAGE: "参考视频 + 参考图",
};

function statusLabel(status: ProjectProductionReadinessStatus): string {
  if (status === "READY") return "✓ 项目可以开工";
  if (status === "PARTIAL") return "⚠ 项目部分路径可以开工";
  if (status === "BUSY") return "⏳ 运行环境忙碌";
  return "✕ 当前项目无法开工";
}

function pathStatusLabel(status: ProjectProductionPathReadiness["status"]): string {
  if (status === "READY") return "可开工";
  if (status === "WARNING") return "可开工，但需要注意";
  return "当前不可开工";
}

function sourceLabel(
  path: ProjectProductionPathReadiness["path"],
  source: ProjectWorkflowPreflightSource | undefined,
): string {
  if (source === "project_mode") return "模式专用";
  if (source === "project_default") return path === "IMAGE" ? "项目图片默认" : "项目视频默认";
  if (source === "recommended") return "系统推荐";
  if (source === "compatible") return "兼容回退";
  return "—";
}

function runtimeConnectionLabel(connection: ComfyPreflightReport["connection"]): string {
  if (connection === "CONNECTED") return "已连接";
  if (connection === "INCOMPATIBLE") return "不兼容";
  return "离线";
}

function runtimeWorkflowLabel(workflow: ComfyPreflightWorkflowItem): string {
  return workflow.name ?? workflow.workflowVersionId ?? "未知工作流";
}

function fingerprint(report: ReturnType<typeof preflightProjectWorkflow>): string {
  return report.items.map((item) => `${item.path}:${item.recipe?.workflowVersionId ?? "NONE"}`).join("|");
}

function formatVram(free?: number | null, total?: number | null): string {
  if (free == null && total == null) return "未知";
  return `${free == null ? "--" : formatFileSize(free)} 空闲 / ${total == null ? "--" : formatFileSize(total)} 总量`;
}

function issueText(issue: ComfyPreflightIssue): string {
  return [
    issue.detail,
    issue.missingNodes?.length ? `缺少节点：${issue.missingNodes.join("、")}` : undefined,
    issue.suggestedAction ? `建议：${issue.suggestedAction}` : undefined,
  ].filter(Boolean).join(" ");
}

function PathReadiness({ path }: { path: ProjectProductionPathReadiness }) {
  const recipe = path.projectWorkflow.recipe;
  return (
    <article
      className={`project-production-readiness-path project-production-readiness-path-${path.status.toLowerCase()}`}
      data-testid="project-production-readiness-path"
    >
      <div className="project-production-readiness-path-heading">
        <strong>{PATH_LABELS[path.path]}</strong>
        <span aria-label={pathStatusLabel(path.status)}>
          {path.status === "READY" ? "✓" : path.status === "WARNING" ? "⚠" : "✕"} {pathStatusLabel(path.status)}
        </span>
      </div>
      {recipe ? (
        <>
          <strong>{workflowDisplayName(recipe.workflowId, recipe.name)}</strong>
          <small>WorkflowVersion：{recipe.workflowVersionId} · Recipe：{recipe.recipeId}</small>
          <small>来源：{sourceLabel(path.path, path.projectWorkflow.source)}</small>
        </>
      ) : null}
      <small>Runtime：{path.runtimeWorkflow?.status ?? "未匹配"}</small>
      {path.runtimeWorkflow && <small>运行包：{runtimeWorkflowLabel(path.runtimeWorkflow)}</small>}
      {path.reasons.map((reason) => <p className="project-production-readiness-reason" key={reason}>{reason}</p>)}
      {path.issues.map((issue, index) => (
        <p className="project-production-readiness-reason" key={`${issue.code}-${index}`}>{issueText(issue)}</p>
      ))}
    </article>
  );
}

function ReadinessReport({ report }: { report: ProjectProductionReadinessReport }) {
  return (
    <>
      <div className={`project-production-readiness-summary project-production-readiness-summary-${report.status.toLowerCase()}`} role="status" aria-live="polite">
        <strong>{statusLabel(report.status)}</strong>
        <span>{report.runnablePathCount} / {report.totalCount} 条生产路径具备开工条件</span>
        {report.warningPathCount > 0 && <span>⚠ {report.warningPathCount} 条路径需要注意</span>}
        {report.blockedPathCount > 0 && <span>{report.blockedPathCount} 条路径当前不可用</span>}
        {report.runtimeBusy && (
          <span>当前已有任务正在运行。待运行环境空闲后，{report.runnablePathCount} / {report.totalCount} 条路径可开工。</span>
        )}
      </div>
      <dl className="project-production-readiness-runtime">
        <div><dt>ComfyUI</dt><dd>{runtimeConnectionLabel(report.connection)}</dd></div>
        <div><dt>运行时</dt><dd>{report.runtimeBusy ? "忙碌" : "空闲"}</dd></div>
        <div><dt>活动任务</dt><dd>{report.activeTaskCount}</dd></div>
        <div><dt>生产队列</dt><dd>{report.productionBusy ? "运行中" : "空闲"}</dd></div>
        <div><dt>GPU</dt><dd>{report.gpu ?? "不可用"}</dd></div>
        <div><dt>VRAM</dt><dd>{formatVram(report.vramFree, report.vramTotal)}</dd></div>
        <div><dt>节点</dt><dd>{report.nodeCount ?? "未知"}</dd></div>
        <div><dt>检查时间</dt><dd>{formatDateTime(report.runtimeCheckedAt)}</dd></div>
      </dl>
      <div className="project-production-readiness-paths">
        {report.paths.map((path) => <PathReadiness key={path.path} path={path} />)}
      </div>
      {report.relevantIssues.length > 0 && (
        <section className="project-production-readiness-issues" aria-labelledby="project-production-readiness-issues-title">
          <h4 id="project-production-readiness-issues-title">当前项目相关运行问题</h4>
          {report.relevantIssues.map((issue, index) => (
            <div className="project-production-readiness-issue" key={`${issue.code}-${index}`}>
              <strong>{issue.title}</strong>
              <p>{issueText(issue)}</p>
            </div>
          ))}
        </section>
      )}
    </>
  );
}

export function ProjectProductionReadiness({ config, catalog }: Props) {
  const projectReport = useMemo(() => preflightProjectWorkflow(config, catalog), [catalog, config]);
  const projectFingerprint = useMemo(() => fingerprint(projectReport), [projectReport]);
  const [readinessReport, setReadinessReport] = useState<ProjectProductionReadinessReport>();
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string>();
  const previousFingerprint = useRef<string | undefined>(undefined);

  useEffect(() => {
    const previous = previousFingerprint.current;
    previousFingerprint.current = projectFingerprint;
    if (previous !== undefined && previous !== projectFingerprint) {
      setReadinessReport(undefined);
      setError("项目工作流已变化，请重新检查开工条件。");
    }
  }, [projectFingerprint]);

  async function checkReadiness() {
    const checkedFingerprint = projectFingerprint;
    setChecking(true);
    setError(undefined);
    try {
      const nextRuntimeReport = await getComfyPreflight();
      if (previousFingerprint.current !== checkedFingerprint) return;
      setReadinessReport(composeProjectProductionReadiness(projectReport, nextRuntimeReport));
    } catch (value) {
      setError(`${readinessReport ? "重新检查失败" : "开工检查失败"}：${toUserMessage(value)}`);
    } finally {
      setChecking(false);
    }
  }

  return (
    <section className="project-production-readiness" aria-labelledby="project-production-readiness-title">
      <div className="section-heading">
        <div>
          <span className="section-label">项目运行状态</span>
          <h3 id="project-production-readiness-title">项目开工就绪</h3>
          <p className="section-description">将项目工作流配置与当前 ComfyUI Runtime 状态进行只读匹配；不会启动生产。</p>
        </div>
        <button type="button" className="primary-action" onClick={() => void checkReadiness()} disabled={checking}>
          {checking ? "正在检查……" : readinessReport ? "重新检查开工条件" : "检查开工条件"}
        </button>
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      {!readinessReport ? (
        <div className="project-production-readiness-unchecked" role="status">
          <strong>尚未检查当前运行环境</strong>
          <p>项目工作流配置已经独立完成。点击检查后，将把当前项目 Workflow 与实时 ComfyUI Runtime 状态进行匹配。</p>
        </div>
      ) : (
        <ReadinessReport report={readinessReport} />
      )}
    </section>
  );
}
