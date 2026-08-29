import { useCallback, useEffect, useRef, useState } from "react";
import {
  getComfyPreflight,
  getProjectCommandCenter,
} from "../../services/tauriClient";
import type { ComfyPreflightReport } from "../../types/settings";
import type { ProductionAuditActivity, ProductionAuditIntegrity, ProductionAuditIssue, ProductionAuditSummary } from "../../types/productionAudit";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ProjectView } from "../../types/project";
import type { ShotView } from "../../types/shot";
import type {
  ProjectCommandCenterAggregate,
  ProjectCommandCenterConsistencyView,
  ProjectCommandCenterPreparationView,
} from "../../types/projectCommandCenter";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, formatFileSize, projectDisplayName } from "../../i18n/statusLabels";
import { deriveShotStatus } from "../shots/shotDomain";
import { shotProgressSummary, type ShotProgressSummary } from "../shots/shotBatchDomain";
import { ProjectImportDryRunWorkspace } from "./ProjectImportDryRunWorkspace";
import "./ProjectCommandCenter.css";

export type ProjectCommandCenterDestination =
  | "studio"
  | "video"
  | "shots"
  | "assets"
  | "tasks"
  | "projects"
  | "workflows"
  | "settings";

export type ProjectCommandDestination = ProjectCommandCenterDestination;

export interface ProjectCommandCenterSceneProgress {
  id: string;
  name: string;
  path: string;
  total: number;
  completed: number;
  percent: number;
  unassigned?: boolean;
}

export interface ProjectCommandCenterIssue {
  id: string;
  severity: "ERROR" | "WARNING" | "INFO";
  title: string;
  detail: string;
  source: "runtime" | "production";
}

export interface ProjectCommandCenterSummary {
  content: {
    shots: number;
    prompts: number;
    assets: number;
    scenes: number;
    configuredShots: number;
  };
  production: {
    active: number;
    completed: number;
    failed: number;
    reviewRequired: number;
  };
  readiness: {
    status: ComfyPreflightReport["status"] | "UNKNOWN";
    label: string;
    connection: string;
    workflowReady: number;
    workflowTotal: number;
  };
  runtime: {
    activeTaskCount: number;
    busy: boolean;
    productionBusy: boolean;
    gpu?: string | null;
    vram: string;
  };
  progress: ShotProgressSummary & { percent: number };
  sceneProgress: ProjectCommandCenterSceneProgress[];
  issues: ProjectCommandCenterIssue[];
}

export interface RecommendedAction {
  label: string;
  detail: string;
  destination: ProjectCommandCenterDestination;
}

export const COMMAND_CENTER_QUICK_ACTIONS: ReadonlyArray<{
  id: string;
  label: string;
  detail: string;
  destination: ProjectCommandCenterDestination;
}> = [
  { id: "create", label: "创作工作台", detail: "开始新的图片或视频创作。", destination: "studio" },
  { id: "shots", label: "生产准备", detail: "检查镜头就绪度并进入手动生产队列。", destination: "shots" },
  { id: "assets", label: "一致性资产", detail: "管理档案、参考集并继续使用项目素材。", destination: "assets" },
  { id: "tasks", label: "任务历史", detail: "查看运行中的任务和结果。", destination: "tasks" },
  { id: "workflows", label: "工作流", detail: "检查运行包和生产就绪状态。", destination: "workflows" },
  { id: "settings", label: "运行时设置", detail: "检查 ComfyUI、GPU 和预检。", destination: "settings" },
];

interface ProjectCommandCenterProps {
  project?: ProjectView;
  onNavigate?: (destination: ProjectCommandCenterDestination) => void;
}

export interface ProjectCommandCenterViewProps {
  project?: ProjectView;
  summary?: ProductionAuditSummary;
  activity?: ProductionAuditActivity[];
  integrity?: ProductionAuditIntegrity;
  shots?: ShotView[];
  structure?: ProductionStructureTree;
  preflight?: ComfyPreflightReport;
  aggregate?: ProjectCommandCenterAggregate;
  loading?: boolean;
  error?: string;
  refreshBusy?: boolean;
  preflightBusy?: boolean;
  busy?: boolean;
  onRefresh?: () => void;
  onRetry?: () => void;
  onRepreflight?: () => void;
  onNavigate?: (destination: ProjectCommandCenterDestination) => void;
  onOpenImport?: () => void;
}

export function ProjectCommandCenter({ project, onNavigate }: ProjectCommandCenterProps) {
  const projectId = project?.id;
  const [summary, setSummary] = useState<ProductionAuditSummary>();
  const [activity, setActivity] = useState<ProductionAuditActivity[]>([]);
  const [integrity, setIntegrity] = useState<ProductionAuditIntegrity>();
  const [shots, setShots] = useState<ShotView[]>([]);
  const [structure, setStructure] = useState<ProductionStructureTree>();
  const [preflight, setPreflight] = useState<ComfyPreflightReport>();
  const [aggregate, setAggregate] = useState<ProjectCommandCenterAggregate>();
  const [loading, setLoading] = useState(Boolean(projectId));
  const [refreshBusy, setRefreshBusy] = useState(false);
  const [preflightBusy, setPreflightBusy] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [error, setError] = useState<string>();
  const requestId = useRef(0);

  const loadSnapshot = useCallback(async () => {
    const currentRequest = ++requestId.current;
    if (!projectId) {
      setLoading(false);
      setSummary(undefined);
      setActivity([]);
      setIntegrity(undefined);
      setShots([]);
      setStructure(undefined);
      setPreflight(undefined);
      setAggregate(undefined);
      return;
    }
    setLoading(true);
    setError(undefined);
    try {
      const nextAggregate = await getProjectCommandCenter(projectId);
      if (currentRequest !== requestId.current) return;
      setAggregate(nextAggregate);
      setSummary(nextAggregate.audit);
      setActivity(nextAggregate.recentActivity);
      setIntegrity({ projectId, health: nextAggregate.audit.health, checkedAt: nextAggregate.audit.checkedAt, issues: nextAggregate.audit.issues });
      setPreflight(nextAggregate.comfy.preflight ?? undefined);
    } catch (loadError: unknown) {
      if (currentRequest === requestId.current) setError(toUserMessage(loadError));
    } finally {
      if (currentRequest === requestId.current) {
        setLoading(false);
      }
    }
  }, [projectId]);

  useEffect(() => {
    setSummary(undefined);
    setActivity([]);
    setIntegrity(undefined);
    setShots([]);
    setStructure(undefined);
    setPreflight(undefined);
    setAggregate(undefined);
    void loadSnapshot();
  }, [loadSnapshot]);

  async function refresh() {
    if (!projectId || loading || refreshBusy || preflightBusy) return;
    setRefreshBusy(true);
    await loadSnapshot();
    setRefreshBusy(false);
  }

  async function repreflight() {
    if (!projectId || loading || refreshBusy || preflightBusy) return;
    setPreflightBusy(true);
    setError(undefined);
    try {
      setPreflight(await getComfyPreflight());
      await loadSnapshot();
    } catch (preflightError: unknown) {
      setError(toUserMessage(preflightError));
    } finally {
      setPreflightBusy(false);
    }
  }

  return (
    <>
      <ProjectCommandCenterView
        project={project}
        summary={summary}
        activity={activity}
        integrity={integrity}
        shots={shots}
        structure={structure}
        preflight={preflight}
        aggregate={aggregate}
        loading={loading}
        error={error}
        refreshBusy={refreshBusy}
        preflightBusy={preflightBusy}
        onRefresh={() => void refresh()}
        onRetry={() => void loadSnapshot()}
        onRepreflight={() => void repreflight()}
        onNavigate={onNavigate}
        onOpenImport={() => setImportOpen(true)}
      />
      {projectId && importOpen && (
        <ProjectImportDryRunWorkspace
          projectId={projectId}
          onClose={() => setImportOpen(false)}
          onImported={() => loadSnapshot()}
        />
      )}
    </>
  );
}

export function ProjectCommandCenterView({
  project,
  summary,
  activity = [],
  integrity,
  shots = [],
  structure,
  preflight,
  aggregate,
  loading = false,
  error,
  refreshBusy = false,
  preflightBusy = false,
  busy = false,
  onRefresh,
  onRetry,
  onRepreflight,
  onNavigate,
  onOpenImport,
}: ProjectCommandCenterViewProps) {
  const derived = aggregate
    ? deriveProjectCommandCenterAggregateSummary(aggregate)
    : deriveProjectCommandCenterSummary(summary, integrity, preflight, shots, structure);
  const displayActivity = aggregate?.recentActivity ?? activity;
  const hasSnapshot = Boolean(aggregate || summary || integrity || preflight || displayActivity.length || shots.length || structure);
  const action = aggregate ? recommendedActionFromAggregate(aggregate.recommendedAction, aggregate) : recommendedAction(derived);
  const busyNow = busy || refreshBusy || preflightBusy || loading;
  const refreshDisabled = busyNow || !onRefresh;
  const preflightDisabled = busyNow || !onRepreflight;

  return (
    <section className="workspace-panel project-command-center" aria-busy={busyNow || undefined}>
      <div className="section-heading workspace-heading project-command-heading">
        <div className="project-command-title-block">
          <span className="section-label">项目指挥中心</span>
          <h2>项目指挥中心</h2>
          <p className="section-description">从项目状态、运行环境到镜头进度，集中决定下一步工作。</p>
        </div>
        <div className="project-command-heading-actions">
          <button type="button" className="quiet-button" onClick={() => onNavigate?.("projects")} disabled={!onNavigate || busyNow}>管理项目</button>
          {project && onOpenImport && <button type="button" className="quiet-button" onClick={onOpenImport} disabled={busyNow}>批量导入预检</button>}
          <button type="button" onClick={onRefresh} disabled={refreshDisabled}>
            {refreshBusy || loading ? "正在刷新……" : "刷新项目"}
          </button>
          <button type="button" className="primary-action" onClick={onRepreflight} disabled={preflightDisabled}>
            {preflightBusy ? "正在预检……" : "重新预检"}
          </button>
        </div>
      </div>

      {project && (
        <section className="project-command-project" aria-labelledby="project-command-project-title">
          <div className="project-command-project-copy">
            <span className="section-label">项目</span>
            <h3 id="project-command-project-title">{projectDisplayName(project.id, project.name)}</h3>
            <p>{project.description?.trim() || "暂无项目说明。"}</p>
            <dl className="project-command-project-meta">
              <div><dt>项目 ID</dt><dd>{project.id}</dd></div>
              <div><dt>更新时间</dt><dd>{formatDateTime(project.updatedAt)}</dd></div>
            </dl>
          </div>
          <div className="project-command-project-next">
            <span className="section-label">继续工作</span>
            <strong>推荐下一步：{action.label}</strong>
            <p>{action.detail}</p>
            <button type="button" className="primary-action" onClick={() => onNavigate?.(action.destination)} disabled={!onNavigate || busyNow} aria-label="继续工作">
              继续工作
            </button>
          </div>
        </section>
      )}

      {loading && !hasSnapshot && <LoadingState />}
      {!loading && error && (
        <section className="project-command-error" role="alert">
          <div><strong>项目指挥中心加载失败</strong><p>{error}</p></div>
          <button type="button" onClick={onRetry} disabled={!onRetry || busyNow}>重试</button>
        </section>
      )}

      {!loading && !hasSnapshot && !error && (
        <section className="project-command-empty" aria-label="项目为空">
          <span className="section-label">项目为空</span>
          <h3>{project ? "当前项目还没有内容" : "暂无项目"}</h3>
          <p>{project ? "先创建镜头或开始一次创作，完成后这里会汇总项目状态。" : "选择一个项目后，这里会显示项目就绪度、生产进度和最近活动。"}</p>
          {!project && <button type="button" onClick={() => onNavigate?.("projects")} disabled={!onNavigate}>管理项目</button>}
        </section>
      )}

      {hasSnapshot && (
        <>
          <div className="project-command-summary-grid" aria-label="项目摘要">
            <SummaryCard label="就绪度" title={derived.readiness.label} tone={derived.readiness.status.toLowerCase()}>
              <span>{derived.readiness.workflowReady} / {derived.readiness.workflowTotal} 个工作流可用</span>
              <small>{derived.readiness.connection}</small>
            </SummaryCard>
            <SummaryCard label="内容" title={`${derived.content.shots} 个镜头`}>
              <span>{derived.content.prompts} 个提示词 · {derived.content.assets} 个素材</span>
              <small>{derived.content.scenes} 个场景 · {derived.content.configuredShots} 个已配置</small>
            </SummaryCard>
            <SummaryCard label="生产" title={`${derived.production.active} 个活动项`} tone={derived.production.failed ? "warning" : undefined}>
              <span>{derived.production.completed} 个运行完成 · {derived.production.failed} 个失败</span>
              <small>{derived.production.reviewRequired} 个待人工检查</small>
            </SummaryCard>
            <SummaryCard label="运行参数" title={derived.runtime.busy ? "运行中" : "空闲"} tone={derived.runtime.busy ? "active" : undefined}>
              <span>{derived.readiness.connection} · {derived.runtime.activeTaskCount} 个活动任务</span>
              <small>{derived.runtime.gpu || "GPU 不可用"} · {derived.runtime.vram}</small>
            </SummaryCard>
          </div>

          <ProjectCommandCenterIntegrationSummary
            consistency={aggregate?.consistency}
            preparation={aggregate?.preparation}
          />

          {!project && (
            <section className="project-command-recommendation" aria-labelledby="project-command-recommendation-title">
              <div>
                <span className="section-label">继续工作</span>
                <h3 id="project-command-recommendation-title">推荐下一步：{action.label}</h3>
                <p>{action.detail}</p>
              </div>
              <button type="button" className="primary-action" onClick={() => onNavigate?.(action.destination)} disabled={!onNavigate || busyNow} aria-label="继续工作">
                继续工作
              </button>
            </section>
          )}

          <div className="project-command-workspace">
            <div className="project-command-main-column">
              <section className="project-command-card project-command-progress-card" aria-labelledby="project-command-progress-title">
                <CardHeading eyebrow="进度" title="项目进度" id="project-command-progress-title" />
                <ProgressBar percent={derived.progress.percent} label={`${derived.progress.completed} / ${derived.progress.total} 个镜头已完成`} />
                <div className="project-command-stat-row">
                  <Stat label="待生成" value={derived.progress.pendingKeyframes} />
                  <Stat label="待选关键帧" value={Math.max(0, derived.progress.keyframesSelected - derived.progress.completed)} />
                  <Stat label="视频复核" value={derived.progress.pendingVideoReview} />
                  <Stat label="需关注" value={derived.progress.needsAttention} />
                </div>
              </section>

              <section className="project-command-card" aria-labelledby="project-command-scene-title">
                <CardHeading eyebrow="场景进度" title="场景进度" id="project-command-scene-title" />
                {derived.sceneProgress.length ? (
                  <div className="project-command-scene-list">
                    {derived.sceneProgress.map((scene) => (
                      <div className="project-command-scene-row" key={scene.id}>
                        <div className="project-command-scene-copy"><strong>{scene.name}</strong><small>{scene.path} · {scene.completed} / {scene.total}</small></div>
                        <ProgressBar percent={scene.percent} label={`${scene.percent}%`} compact />
                      </div>
                    ))}
                  </div>
                ) : <p className="project-command-muted">尚未建立场景结构。</p>}
              </section>
            </div>

            <aside className="project-command-side-column" aria-label="项目上下文">
              <section className="project-command-card" aria-labelledby="project-command-issues-title">
                <CardHeading eyebrow="问题" title="需要关注" id="project-command-issues-title" />
                {derived.issues.length ? (
                  <div className="project-command-issue-list">
                    {derived.issues.map((issue) => <article className={`project-command-issue project-command-issue-${issue.severity.toLowerCase()}`} key={issue.id}><div><strong>{issue.title}</strong><span>{issue.source === "runtime" ? "运行时" : "生产数据"}</span></div><p>{issue.detail}</p></article>)}
                  </div>
                ) : <p className="project-command-healthy">未发现需要处理的问题。</p>}
              </section>

              <section className="project-command-card" aria-labelledby="project-command-activity-title">
                <CardHeading eyebrow="最近活动" title="最近活动" id="project-command-activity-title" />
                {displayActivity.length ? (
                  <div className="project-command-activity-list">
                    {displayActivity.slice(0, 8).map((item) => <article className={`project-command-activity project-command-activity-${item.severity.toLowerCase()}`} key={item.id}><time dateTime={item.timestamp}>{formatDateTime(item.timestamp)}</time><div><strong>{item.title}</strong><p>{item.detail}</p>{item.errorCode && <code>{item.errorCode}</code>}</div></article>)}
                  </div>
                ) : <p className="project-command-muted">当前项目还没有生产活动。</p>}
              </section>

              <section className="project-command-card project-command-quick-actions" aria-labelledby="project-command-quick-actions-title">
                <CardHeading eyebrow="快速操作" title="快速操作" id="project-command-quick-actions-title" />
                <div className="project-command-action-grid">
                  {COMMAND_CENTER_QUICK_ACTIONS.slice(0, 3).map((item) => <button type="button" className="project-command-action" data-command-action={item.id} key={item.id} onClick={() => onNavigate?.(item.destination)} disabled={!onNavigate || busyNow}><strong>{item.label}</strong><span>{item.detail}</span></button>)}
                </div>
                <small className="project-command-quick-actions-hint">其他管理入口已收进左侧“管理与设置”。</small>
              </section>
            </aside>
          </div>
        </>
      )}
    </section>
  );
}

export function deriveProjectCommandCenterSummary(
  summary: ProductionAuditSummary | undefined,
  integrity: ProductionAuditIntegrity | undefined,
  preflight: ComfyPreflightReport | undefined,
  shots: ShotView[] = [],
  structure: ProductionStructureTree | undefined,
): ProjectCommandCenterSummary {
  const progress = shotProgressSummary(shots);
  const sceneProgress = buildSceneProgress(structure, shots);
  const issues = buildProjectCommandCenterIssues(summary, integrity, preflight);
  const workflowSummary = preflight?.workflowSummary;
  return {
    content: {
      shots: shots.length,
      prompts: shots.filter((shot) => Boolean(shot.promptText.trim())).length,
      assets: summary?.assets ?? 0,
      scenes: sceneProgress.filter((scene) => !scene.unassigned).length,
      configuredShots: shots.filter((shot) => shot.stageConfigs.length > 0).length,
    },
    production: {
      active: (summary?.activeRuns ?? 0) + (summary?.activeBatches ?? 0),
      completed: summary?.completedRuns ?? 0,
      failed: (summary?.failedRuns ?? 0) + (summary?.failedBatches ?? 0) + (summary?.failedItems ?? 0) + (summary?.failedTasks ?? 0),
      reviewRequired: summary?.reviewRequiredItems ?? 0,
    },
    readiness: {
      status: preflight?.status ?? "UNKNOWN",
      label: preflightStatusLabel(preflight?.status),
      connection: preflight ? connectionLabel(preflight.connection) : "尚未预检",
      workflowReady: workflowSummary?.workflowReady ?? 0,
      workflowTotal: workflowSummary?.workflowTotal ?? 0,
    },
    runtime: {
      activeTaskCount: preflight?.activeTaskCount ?? 0,
      busy: Boolean(preflight?.runtimeBusy || preflight?.productionBusy),
      productionBusy: Boolean(preflight?.productionBusy),
      gpu: preflight?.gpu,
      vram: preflight ? formatVram(preflight.vramFree, preflight.vramTotal) : "尚未预检",
    },
    progress: { ...progress, percent: progress.total ? Math.round((progress.completed / progress.total) * 100) : 0 },
    sceneProgress,
    issues,
  };
}

export function deriveProjectCommandCenterAggregateSummary(aggregate: ProjectCommandCenterAggregate): ProjectCommandCenterSummary {
  const { shots, queue, structure, comfy, audit } = aggregate;
  const preflight = comfy.preflight ?? undefined;
  const issues = aggregate.issues.length
    ? aggregate.issues.map((issue) => ({
      ...issue,
      severity: (issue.severity === "ERROR" || issue.severity === "WARNING" ? issue.severity : "INFO") as ProjectCommandCenterIssue["severity"],
      source: (issue.source === "runtime" ? "runtime" : "production") as ProjectCommandCenterIssue["source"],
    }))
    : buildProjectCommandCenterIssues(audit, { projectId: audit.projectId, health: audit.health, checkedAt: audit.checkedAt, issues: audit.issues }, preflight);
  return {
    content: aggregate.content,
    production: aggregate.production,
    readiness: {
      status: (aggregate.readiness.status ?? preflight?.status ?? "UNKNOWN") as ComfyPreflightReport["status"] | "UNKNOWN",
      label: preflightStatusLabel((aggregate.readiness.status ?? preflight?.status) as ComfyPreflightReport["status"] | undefined),
      connection: aggregate.readiness.connection ? connectionLabel(aggregate.readiness.connection as ComfyPreflightReport["connection"]) : "尚未预检",
      workflowReady: aggregate.readiness.workflowReady,
      workflowTotal: aggregate.readiness.workflowTotal,
    },
    runtime: {
      activeTaskCount: aggregate.readiness.activeTaskCount,
      busy: Boolean(aggregate.readiness.runtimeBusy || aggregate.readiness.productionBusy || queue.activeItems > 0),
      productionBusy: Boolean(aggregate.readiness.productionBusy || queue.activeItems > 0),
      gpu: preflight?.gpu ?? comfy.status?.devices[0]?.name,
      vram: preflight ? formatVram(preflight.vramFree, preflight.vramTotal) : "尚未预检",
    },
    progress: {
      total: shots.total,
      pendingKeyframes: shots.draft + shots.ready,
      keyframesSelected: shots.imageSelected + shots.videoReview + shots.completed,
      videoGenerating: 0,
      pendingVideoReview: shots.videoReview,
      completed: shots.completed,
      needsAttention: shots.failed,
      percent: shots.total ? Math.round((shots.completed / shots.total) * 100) : 0,
    },
    sceneProgress: [
      ...structure.scenes.map((scene) => ({ ...scene, percent: scene.total ? Math.round((scene.completed / scene.total) * 100) : 0 })),
      ...(structure.unassignedShotCount > 0 ? [{ id: "unassigned", name: "未分配镜头", path: "项目结构", total: structure.unassignedShotCount, completed: 0, percent: 0, unassigned: true }] : []),
    ],
    issues,
  };
}

function recommendedActionFromAggregate(
  action: ProjectCommandCenterAggregate["recommendedAction"],
  aggregate: ProjectCommandCenterAggregate,
): RecommendedAction {
  const consistencyAction = consistencyRecommendedAction(aggregate);
  if (consistencyAction) return consistencyAction;
  const actions: Record<string, RecommendedAction> = {
    STRUCTURAL_BLOCKED: { label: "修复项目结构", detail: "项目结构或生产链路存在阻断，先处理结构问题。", destination: "shots" },
    COMFY_BLOCKED: { label: "修复运行环境", detail: "当前 ComfyUI 或生产工作流被阻断，先完成运行时预检。", destination: "settings" },
    REVIEW_REQUIRED: { label: "处理生产复核", detail: "有失败或不可自动恢复的生产项需要人工检查。", destination: "shots" },
    AUTO_RESUMABLE: { label: "恢复生产", detail: "有可自动恢复的生产项，继续处理未完成工作。", destination: "shots" },
    ACTIVE_PRODUCTION: { label: "查看运行进度", detail: "项目仍有任务或生产批次活动中，先确认当前进度。", destination: "tasks" },
    IMAGE_REVIEW: { label: "完成图片复核", detail: "有关键帧候选等待人工确认。", destination: "shots" },
    VIDEO_REVIEW: { label: "完成视频复核", detail: "有视频候选等待人工确认。", destination: "shots" },
    MISSING_CONFIG: { label: "配置下一镜头", detail: "还有镜头缺少工作流或配方配置。", destination: "shots" },
    UNASSIGNED: { label: "整理项目结构", detail: "还有镜头尚未分配到场景。", destination: "shots" },
    NO_SHOTS: { label: "建立第一个镜头", detail: "项目还没有镜头，从镜头生产工作区建立可追踪的制作单元。", destination: "shots" },
    READY: { label: "继续创作", detail: "项目已经准备好进入下一步生产。", destination: "shots" },
    COMPLETE: { label: "开始新一轮创作", detail: "当前镜头已完成，可以回到创作工作台开始新的内容。", destination: "studio" },
  };
  return actions[action.kind] ?? { label: "继续工作", detail: action.reason, destination: "shots" };
}

function consistencyRecommendedAction(aggregate: ProjectCommandCenterAggregate): RecommendedAction | undefined {
  const consistency = aggregate.consistency;
  if (!consistency?.consistencyInUse) return undefined;
  const profileCount = consistency.characterProfiles + consistency.sceneProfiles + consistency.propProfiles + consistency.styleProfiles;
  const bindingCount = consistency.shotProfileBindings + consistency.shotReferenceSetBindings + consistency.scopeProfileBindings + consistency.scopeReferenceSetBindings;
  if (profileCount > 0 && bindingCount === 0) {
    return { label: "配置镜头一致性", detail: "已有一致性档案或参考集，先为镜头配置绑定再进入生产。", destination: "shots" };
  }
  if (bindingCount > 0 && aggregate.preparation && aggregate.preparation.snapshotCount === 0 && aggregate.shots.total > 0) {
    return { label: "生产准备", detail: "绑定已就绪，先为镜头生成生产准备快照，再手动加入队列。", destination: "shots" };
  }
  return undefined;
}

function ProjectCommandCenterIntegrationSummary({
  consistency,
  preparation,
}: {
  consistency?: ProjectCommandCenterConsistencyView;
  preparation?: ProjectCommandCenterPreparationView;
}) {
  const current = consistency ?? {
    characterProfiles: 0,
    sceneProfiles: 0,
    propProfiles: 0,
    styleProfiles: 0,
    referenceSets: 0,
    shotProfileBindings: 0,
    shotReferenceSetBindings: 0,
    scopeProfileBindings: 0,
    scopeReferenceSetBindings: 0,
    consistencyInUse: false,
  } satisfies ProjectCommandCenterConsistencyView;
  const shotBindings = current.shotProfileBindings + current.shotReferenceSetBindings;
  const scopeBindings = current.scopeProfileBindings + current.scopeReferenceSetBindings;

  return (
    <div className="project-command-columns" aria-label="一致性与生产准备" role="region">
      <section className="project-command-card" aria-labelledby="project-command-consistency-title">
        <CardHeading eyebrow="一致性" title="一致性摘要" id="project-command-consistency-title" />
        <p className={current.consistencyInUse ? "project-command-healthy" : "project-command-muted"}>
          {current.consistencyInUse ? "已启用" : "未启用（兼容旧项目）"} · 不阻塞现有生产
        </p>
        <div className="project-command-stat-row">
          <Stat label="角色档案" value={current.characterProfiles} />
          <Stat label="场景档案" value={current.sceneProfiles} />
          <Stat label="道具档案" value={current.propProfiles} />
          <Stat label="风格档案" value={current.styleProfiles} />
        </div>
        <small className="project-command-muted">参考集 {current.referenceSets} · 范围绑定 {scopeBindings} · 镜头绑定 {shotBindings}</small>
      </section>

      <section className="project-command-card" aria-labelledby="project-command-preparation-title">
        <CardHeading eyebrow="生产准备" title="生产准备摘要" id="project-command-preparation-title" />
        {preparation ? (
          <>
            <div className="project-command-stat-row">
              <Stat label="已准备图片" value={preparation.preparedImageItems} />
              <Stat label="已准备视频" value={preparation.preparedVideoItems} />
              <Stat label="快照数" value={preparation.snapshotCount} />
              <Stat label="活动准备" value={preparation.activePreparedItems} />
            </div>
            <small className="project-command-muted">最近准备：{preparation.latestPreparedAt ? formatDateTime(preparation.latestPreparedAt) : "暂无"}</small>
          </>
        ) : <p className="project-command-muted">当前后端未提供准备快照摘要；旧项目仍可按原流程生产。</p>}
      </section>
    </div>
  );
}

export function buildSceneProgress(structure: ProductionStructureTree | undefined, shots: ShotView[]): ProjectCommandCenterSceneProgress[] {
  if (!structure) return [];
  const shotsById = new Map(shots.map((shot) => [shot.id, shot]));
  const result: ProjectCommandCenterSceneProgress[] = [];
  for (const series of structure.series) {
    for (const episode of series.episodes) {
      for (const scene of episode.scenes) {
        const total = scene.shotIds.length;
        const completed = scene.shotIds.reduce((count, shotId) => count + (shotsById.get(shotId) && deriveShotStatus(shotsById.get(shotId)!) === "COMPLETED" ? 1 : 0), 0);
        result.push({
          id: scene.id,
          name: scene.name,
          path: `${series.name} / ${episode.name}`,
          total,
          completed,
          percent: total ? Math.round((completed / total) * 100) : 0,
        });
      }
    }
  }
  if (structure.unassignedShotIds.length) {
    const total = structure.unassignedShotIds.length;
    const completed = structure.unassignedShotIds.reduce((count, shotId) => count + (shotsById.get(shotId) && deriveShotStatus(shotsById.get(shotId)!) === "COMPLETED" ? 1 : 0), 0);
    result.push({ id: "unassigned", name: "未分配镜头", path: "项目结构", total, completed, percent: Math.round((completed / total) * 100), unassigned: true });
  }
  return result;
}

export function buildProjectCommandCenterIssues(
  summary: ProductionAuditSummary | undefined,
  integrity: ProductionAuditIntegrity | undefined,
  preflight: ComfyPreflightReport | undefined,
): ProjectCommandCenterIssue[] {
  const issues: ProjectCommandCenterIssue[] = [];
  const seen = new Set<string>();
  const add = (issue: ProjectCommandCenterIssue) => {
    const key = `${issue.source}:${issue.title}:${issue.detail}`;
    if (seen.has(key)) return;
    seen.add(key);
    issues.push(issue);
  };
  for (const issue of preflight?.issues ?? []) add({ id: `runtime:${issue.code}:${issue.workflowId ?? ""}`, severity: issue.severity, title: issue.title, detail: `${issue.detail}${issue.suggestedAction ? ` 建议：${issue.suggestedAction}` : ""}`, source: "runtime" });
  for (const issue of [...(integrity?.issues ?? []), ...(summary?.issues ?? [])]) add(auditIssue(issue));
  return issues;
}

export function recommendedAction(summary: ProjectCommandCenterSummary): RecommendedAction {
  if (summary.readiness.status === "BLOCKED") return { label: "修复运行环境", detail: "当前 ComfyUI 或生产工作流被阻断，先完成运行时预检。", destination: "settings" };
  if (summary.issues.some((issue) => issue.severity === "ERROR")) return { label: "处理生产问题", detail: "项目存在失败或断链记录，先检查问题再继续生产。", destination: "shots" };
  if (summary.progress.needsAttention > 0) return { label: "处理失败镜头", detail: "有镜头生成失败，打开镜头生产工作区继续处理。", destination: "shots" };
  if (summary.runtime.activeTaskCount > 0 || summary.production.active > 0) return { label: "查看运行进度", detail: "项目仍有任务或生产批次活动中，先确认当前进度。", destination: "tasks" };
  if (summary.progress.total === 0) return { label: "建立第一个镜头", detail: "项目还没有镜头，从镜头生产工作区建立可追踪的制作单元。", destination: "shots" };
  if (summary.progress.pendingVideoReview > 0) return { label: "完成视频复核", detail: "有视频候选等待人工确认，完成选择后再进入下一步。", destination: "shots" };
  if (summary.progress.completed === summary.progress.total) return { label: "开始新一轮创作", detail: "当前镜头已完成，可以回到创作工作台开始新的内容。", destination: "studio" };
  if (summary.progress.pendingKeyframes > 0) return { label: "生成关键帧", detail: "还有镜头等待关键帧生成，继续完成项目的图像阶段。", destination: "shots" };
  return { label: "继续创作", detail: "回到创作工作台继续使用当前项目。", destination: "studio" };
}

function auditIssue(issue: ProductionAuditIssue): ProjectCommandCenterIssue {
  return { id: `production:${issue.code}:${issue.entityType}:${issue.entityId}`, severity: issue.severity, title: issue.code, detail: `${issue.message} · ${issue.entityType} ${issue.entityId}`, source: "production" };
}

function SummaryCard({ label, title, tone, children }: { label: string; title: string; tone?: string; children: React.ReactNode }) {
  return <article className={`project-command-summary-card${tone ? ` project-command-summary-${tone}` : ""}`}><span className="section-label">{label}</span><strong>{title}</strong><div>{children}</div></article>;
}

function CardHeading({ eyebrow, title, id }: { eyebrow: string; title: string; id: string }) {
  return <div className="project-command-card-heading"><div><span className="section-label">{eyebrow}</span><h3 id={id}>{title}</h3></div></div>;
}

function ProgressBar({ percent, label, compact = false }: { percent: number; label: string; compact?: boolean }) {
  return <div className={`project-command-progress${compact ? " project-command-progress-compact" : ""}`}><div className="project-command-progress-line"><span>{label}</span><strong>{percent}%</strong></div><div className="project-command-progress-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-label={label}><span style={{ width: `${percent}%` }} /></div></div>;
}

function Stat({ label, value }: { label: string; value: number }) {
  return <div className="project-command-stat"><span>{label}</span><strong>{value}</strong></div>;
}

function LoadingState() {
  return <div className="project-command-loading" role="status"><span className="project-command-loading-pulse" aria-hidden="true" /><div><strong>正在加载项目状态……</strong><p>正在读取项目、生产记录、镜头结构和运行环境。</p></div></div>;
}

function preflightStatusLabel(status: ComfyPreflightReport["status"] | undefined): string {
  if (status === "READY") return "运行环境就绪";
  if (status === "WARNING") return "运行环境需关注";
  if (status === "BLOCKED") return "运行环境已阻断";
  return "尚未预检";
}

function connectionLabel(connection: ComfyPreflightReport["connection"]): string {
  if (connection === "CONNECTED") return "ComfyUI 已连接";
  if (connection === "INCOMPATIBLE") return "ComfyUI 不兼容";
  return "ComfyUI 离线";
}

function formatVram(free?: number | null, total?: number | null): string {
  if (free == null && total == null) return "VRAM 未知";
  return `${free == null ? "--" : formatFileSize(free)} 空闲 / ${total == null ? "--" : formatFileSize(total)}`;
}
