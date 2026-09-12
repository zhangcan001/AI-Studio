// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  buildSceneProgress,
  deriveProjectCommandCenterSummary,
  ProjectCommandCenterView,
  recommendedAction,
} from "./ProjectCommandCenter";
import type { ComfyPreflightReport } from "../../types/settings";
import type { ProjectView } from "../../types/project";
import type { ProductionAuditIntegrity, ProductionAuditSummary } from "../../types/productionAudit";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ProjectCommandCenterAggregate } from "../../types/projectCommandCenter";
import type { ShotView } from "../../types/shot";
import type { TaskView } from "../../types/task";

const project: ProjectView = {
  id: "project-1",
  name: "Long project name that should remain readable even when the workspace is narrow",
  description: "A project description that is deliberately long so the command center keeps text inside cards instead of overflowing the layout.",
  createdAt: "2026-08-18T08:00:00Z",
  updatedAt: "2026-08-18T10:00:00Z",
};

function task(id: string, status: TaskView["status"]): TaskView {
  return { id, projectId: project.id, status, progress: { mode: "indeterminate" }, createdAt: "2026-08-18T09:00:00Z", outputAssetIds: [] };
}

function shot(id: string, status: "complete" | "review" | "failed" | "running" | "draft"): ShotView {
  const taskStatus = status === "review" ? "SUCCEEDED" : status === "failed" ? "FAILED" : status === "running" ? "RUNNING" : undefined;
  return {
    id,
    projectId: project.id,
    ordinal: Number(id.replace(/\D/g, "")) || 1,
    name: `Shot ${id}`,
    promptText: "A test shot",
    selectedImageAssetId: status === "complete" ? `asset-${id}` : undefined,
    createdAt: "2026-08-18T08:00:00Z",
    updatedAt: "2026-08-18T09:00:00Z",
    status: "DRAFT",
    imageStatus: "DRAFT",
    videoStatus: "DRAFT",
    stageConfigs: status === "draft" ? [] : [{ stage: "image", workflowVersionId: "workflow-1", recipeId: "recipe-1", scalarValues: {}, updatedAt: "2026-08-18T08:00:00Z" }],
    referenceAssets: [],
    generationLinks: taskStatus ? [{ id: `link-${id}`, stage: "image", task: task(`task-${id}`, taskStatus), createdAt: "2026-08-18T09:00:00Z" }] : [],
  };
}

const structure: ProductionStructureTree = {
  projectId: project.id,
  series: [{
    id: "series-1", projectId: project.id, ordinal: 0, name: "Series A", description: "", createdAt: "2026-08-18T08:00:00Z", updatedAt: "2026-08-18T08:00:00Z",
    episodes: [{
      id: "episode-1", seriesId: "series-1", ordinal: 0, name: "Episode 1", description: "", createdAt: "2026-08-18T08:00:00Z", updatedAt: "2026-08-18T08:00:00Z",
      scenes: [{ id: "scene-1", episodeId: "episode-1", ordinal: 0, name: "Opening", description: "", shotIds: ["shot-1", "shot-2"], createdAt: "2026-08-18T08:00:00Z", updatedAt: "2026-08-18T08:00:00Z" }],
    }],
  }],
  unassignedShotIds: ["shot-3"],
};

const preflight: ComfyPreflightReport = {
  endpoint: "http://127.0.0.1:8188", status: "READY", checkedAt: "2026-08-18T10:00:00Z", connection: "CONNECTED", comfyuiVersion: "0.33.0", pythonVersion: "3.12.10", gpu: "NVIDIA Test GPU", vramTotal: 16 * 1024 ** 3, vramFree: 8 * 1024 ** 3, nodeCount: 4516, runtimeBusy: false, activeTaskCount: 0, productionBusy: false,
  workflowSummary: { workflowTotal: 3, workflowReady: 3, workflowBlocked: 0 }, issues: [],
};

function audit(overrides: Partial<ProductionAuditSummary> = {}): ProductionAuditSummary {
  return {
    projectId: project.id, health: "HEALTHY", activeRuns: 0, completedRuns: 1, failedRuns: 0, activeBatches: 0, pausedBatches: 0, failedBatches: 0, logicalItems: 3, attempts: 3, succeededItems: 3, failedItems: 0, reviewRequiredItems: 0, tasks: 3, succeededTasks: 3, failedTasks: 0, assets: 12, unassignedShots: 1, checkedAt: "2026-08-18T10:00:00Z", issues: [], ...overrides,
  };
}

const integrity: ProductionAuditIntegrity = { projectId: project.id, health: "HEALTHY", issues: [], checkedAt: "2026-08-18T10:00:00Z" };

afterEach(cleanup);

function viewProps(shots: ShotView[] = [shot("shot-1", "complete"), shot("shot-2", "review"), shot("shot-3", "draft")], overrides: { summary?: ProductionAuditSummary; integrity?: ProductionAuditIntegrity; preflight?: ComfyPreflightReport } = {}) {
  return {
    project,
    summary: overrides.summary ?? audit(),
    integrity: overrides.integrity ?? integrity,
    shots,
    structure,
    preflight: overrides.preflight ?? preflight,
    activity: [{ id: "activity-1", kind: "TASK_SUCCEEDED", timestamp: "2026-08-18T09:00:00Z", severity: "INFO" as const, title: "任务已完成", detail: "A very long activity detail that must wrap within the activity card rather than pushing the page horizontally." }],
  };
}

function aggregate(overrides: Partial<ProjectCommandCenterAggregate> = {}): ProjectCommandCenterAggregate {
  return {
    project: { ...project },
    structure: { seriesCount: 1, episodeCount: 1, sceneCount: 1, assignedShotCount: 2, unassignedShotCount: 0, firstUnassignedShotId: null, blocked: false, scenes: [{ id: "scene-1", name: "Opening", path: "Series A / Episode 1", total: 2, completed: 1 }] },
    shots: { total: 2, draft: 0, ready: 0, generating: 0, imageReview: 0, imageSelected: 0, videoReview: 1, completed: 1, failed: 0, configured: 2, missingConfig: 0, firstGeneratingShotId: null, firstImageReviewShotId: null, firstVideoReviewShotId: "shot-2", firstMissingConfigShotId: null, firstReadyShotId: null },
    queue: { totalQueues: 1, runningQueues: 0, pausedQueues: 0, completedQueues: 1, archivedQueues: 0, totalItems: 2, pendingItems: 0, activeItems: 0, succeededItems: 2, failedItems: 0, cancelledItems: 0, skippedItems: 0, autoResumableItems: 0, reviewRequiredItems: 0, firstActiveBatchId: null, firstAutoResumableBatchId: null, firstReviewRequiredBatchId: null },
    tasksAssets: { taskCount: 2, activeTaskCount: 0, succeededTaskCount: 2, failedTaskCount: 0, assetCount: 4, imageAssetCount: 2, videoAssetCount: 2, audioAssetCount: 0, otherAssetCount: 0 },
    referenceAnchors: { total: 0, usable: 0, character: 0, scene: 0, prop: 0, style: 0 },
    promptLibrary: { total: 0, versions: 0, items: [] },
    comfy: { status: null, preflight },
    readiness: { status: "READY", connection: "CONNECTED", workflowReady: 1, workflowTotal: 1, runtimeBusy: false, activeTaskCount: 0, productionBusy: false },
    content: { shots: 2, prompts: 2, assets: 4, scenes: 1, configuredShots: 2 },
    production: { active: 0, completed: 1, failed: 0, reviewRequired: 0 },
    issues: [],
    audit: audit(),
    recentActivity: [],
    recommendedAction: { kind: "VIDEO_REVIEW", priority: 7, reasonCode: "VIDEO_REVIEW", reason: "video review required", shotId: "shot-2", batchId: "batch-1" },
    quickActions: [],
    checkedAt: "2026-08-18T10:00:00Z",
    ...overrides,
  };
}

describe("ProjectCommandCenter", () => {
  it("renders project, progress, review, issues, runtime, and summary surfaces", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps()} />);
    expect(html).toContain("项目总览");
    expect(html).toContain(project.name);
    expect(html).toContain("总体进度");
    expect(html).toContain("待审核");
    expect(html).toContain("需要处理");
    expect(html).toContain("运行环境");
    expect(html).toContain("继续工作");
  });

  it("keeps Continue Work navigation-only and chooses review work deterministically", () => {
    const props = viewProps();
    const derived = deriveProjectCommandCenterSummary(props.summary, props.integrity, props.preflight, props.shots, props.structure);
    const navigate = vi.fn();
    expect(recommendedAction(derived)).toMatchObject({ destination: "shots", label: "生成关键帧" });
    renderToStaticMarkup(<ProjectCommandCenterView {...props} onNavigate={navigate} />);
    expect(navigate).not.toHaveBeenCalled();
  });

  it("renders no more than six quick actions", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps()} onNavigate={vi.fn()} />);
    expect(html.match(/class="project-command-action"/g)?.length).toBeLessThanOrEqual(6);
  });

  it("renders an empty project state", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView onNavigate={vi.fn()} />);
    expect(html).toContain("暂无项目");
    expect(html).toContain("管理项目");
  });

  it("offers a single creative entry point for an active empty project", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} onNavigate={onNavigate} />);

    expect(screen.getByRole("button", { name: "开始创作" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "开始创作" }));

    expect(onNavigate).toHaveBeenCalledWith({ destination: "shots", section: "creation", actionKind: "NO_SHOTS" });
  });

  it("renders complete progress and recommends a next creative round", () => {
    const props = viewProps([shot("shot-1", "complete"), shot("shot-2", "complete")], { summary: audit(), integrity: { ...integrity, issues: [] } });
    const derived = deriveProjectCommandCenterSummary(props.summary, props.integrity, props.preflight, props.shots, props.structure);
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...props} />);
    expect(derived.progress.percent).toBe(100);
    expect(recommendedAction(derived).destination).toBe("assets");
    expect(html).toContain("100%");
    expect(html).toContain("查看交付结果");
  });

  it("shows project and runtime issues without dropping long details", () => {
    const props = viewProps([], {
      summary: audit({ health: "BLOCKED", issues: [{ severity: "ERROR", code: "SHOT_FAILED", message: "A project issue with a long explanation that should wrap safely.", entityType: "SHOT", entityId: "shot-1", relatedIds: [] }] }),
      integrity: { ...integrity, health: "BLOCKED" },
      preflight: { ...preflight, status: "WARNING", issues: [{ severity: "WARNING", code: "MISSING_NODE", title: "缺少节点", detail: "Runtime detail", suggestedAction: "安装节点后重新预检", missingNodes: ["TestNode"] }] },
    });
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...props} />);
    expect(html).toContain("需要关注");
    expect(html).toContain("A project issue with a long explanation");
    expect(html).toContain("缺少节点");
    expect(html).toContain("安装节点后重新预检");
  });

  it("derives progress and scene progress for 500 shots without a rendered shot row per item", () => {
    const shots = Array.from({ length: 500 }, (_, index) => shot(`shot-${index + 1}`, index < 250 ? "complete" : "draft"));
    const derived = deriveProjectCommandCenterSummary(audit(), integrity, preflight, shots, structure);
    expect(derived.progress).toMatchObject({ total: 500, completed: 250, percent: 50 });
    expect(buildSceneProgress(structure, shots)).toHaveLength(2);
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps(shots)} />);
    expect(html).toContain("500 个镜头");
    expect(html).not.toContain("Shot shot-500");
  });

  it("renders recent activity and scene progress", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps()} />);
    expect(html).toContain("最近活动");
    expect(html).toContain("任务已完成");
    expect(html).toContain("Opening");
    expect(html).toContain("场景进度");
  });

  it("shows explicit refresh busy state", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps()} refreshBusy onRefresh={vi.fn()} />);
    expect(html).toContain("正在刷新……");
    expect(html).toContain('aria-busy="true"');
    expect(html).toContain("disabled");
  });

  it("shows explicit preflight busy state", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView {...viewProps()} preflightBusy onRepreflight={vi.fn()} />);
    expect(html).toContain("正在预检……");
    expect(html).toContain('aria-busy="true"');
    expect(html).toContain("disabled");
  });

  it("offers retry for a failed initial load", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView project={project} error="读取项目状态失败" onRetry={vi.fn()} />);
    expect(html).toContain("项目总览加载失败");
    expect(html).toContain("读取项目状态失败");
    expect(html).toContain("重试");
  });

  it("keeps legacy projects non-blocking and shows consistency plus preparation summaries", () => {
    const { rerender } = render(<ProjectCommandCenterView project={project} aggregate={aggregate()} />);
    expect(screen.getByRole("region", { name: "一致性与生产准备" }).textContent).toContain("未启用");
    expect(screen.getByRole("region", { name: "一致性与生产准备" }).textContent).toContain("不阻塞现有生产");

    rerender(
      <ProjectCommandCenterView
        project={project}
        aggregate={aggregate({
          consistency: { characterProfiles: 2, sceneProfiles: 1, propProfiles: 1, styleProfiles: 1, referenceSets: 3, shotProfileBindings: 2, shotReferenceSetBindings: 3, scopeProfileBindings: 4, scopeReferenceSetBindings: 2, consistencyInUse: true },
          preparation: { snapshotCount: 4, preparedImageItems: 3, preparedVideoItems: 1, activePreparedItems: 2, latestPreparedAt: "2026-08-18T09:30:00Z" },
        })}
      />,
    );
    const summary = screen.getByRole("region", { name: "一致性与生产准备" });
    expect(summary.textContent).toContain("已启用");
    expect(summary.textContent).toContain("角色档案");
    expect(summary.textContent).toContain("生产准备");
    expect(summary.textContent).toContain("3");
    expect(summary.textContent).toContain("4");
  });

  it("offers existing Assets and Production destinations from compact quick actions", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate()} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: /一致性资产/ }));
    await user.click(screen.getByRole("button", { name: /生产准备/ }));

    expect(onNavigate.mock.calls).toEqual([[{ destination: "assets" }], [{ destination: "shots" }]]);
  });

  it("renders the derived daily production board and preserves exact repair targets", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    const emptyBucket = { totalCount: 0, items: [], hasMore: false };
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      dailyProduction: {
        needsAttention: {
          totalCount: 1,
          hasMore: false,
          items: [{
            id: "queue:item-1",
            label: "Shot 1",
            reasonCode: "QUEUE_FAILED",
            reason: "生产队列项失败，需要人工检查。",
            severity: "ERROR",
            destination: "tasks",
            stage: "video",
            shotId: "shot-1",
            batchId: "batch-1",
            taskId: "task-1",
            workflowVersionId: "workflow-version-1",
            recipeId: "recipe-1",
          }],
        },
        ready: emptyBucket,
        running: emptyBucket,
        review: emptyBucket,
        completed: emptyBucket,
        topAction: null,
      },
    })} onNavigate={onNavigate} />);

    expect(screen.getByRole("region", { name: "生产行动板" }).textContent).toContain("每日生产");
    await user.click(screen.getByRole("button", { name: /Shot 1.*生产队列项失败/ }));
    expect(onNavigate).toHaveBeenCalledWith({
      destination: "tasks",
      section: "production",
      actionKind: "QUEUE_FAILED",
      shotId: "shot-1",
      batchId: "batch-1",
      taskId: "task-1",
    });
  });

  it("opens large project status collections through existing list filters", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      shots: { ...aggregate().shots, ready: 128, completed: 21 },
      tasksAssets: { ...aggregate().tasksAssets, activeTaskCount: 25, failedTaskCount: 37 },
    })} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "查看全部 37" }));
    await user.click(screen.getByRole("button", { name: "查看全部 25" }));
    await user.click(screen.getByRole("button", { name: "查看全部 128" }));
    await user.click(screen.getByRole("button", { name: "查看全部 21" }));

    expect(onNavigate.mock.calls).toEqual([
      [{ destination: "tasks", collectionFilter: { kind: "tasks", status: "FAILED" }, projectId: "project-1" }],
      [{ destination: "tasks", collectionFilter: { kind: "tasks", status: "ACTIVE" }, projectId: "project-1" }],
      [{ destination: "shots", section: "production", collectionFilter: { kind: "shots", status: "READY" }, projectId: "project-1" }],
      [{ destination: "shots", section: "creation", collectionFilter: { kind: "shots", status: "COMPLETED" }, projectId: "project-1" }],
    ]);
  });

  it("offers a bounded 500-shot unassigned collection without rendering all rows", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    const items = Array.from({ length: 20 }, (_, index) => ({
      id: `shot:${index}`,
      label: `Shot ${index}`,
      reasonCode: "UNASSIGNED_SHOT",
      reason: "镜头尚未分配到项目结构。",
      severity: "ERROR",
      destination: "shots",
      stage: null,
      shotId: `shot-${index}`,
      batchId: null,
      taskId: null,
      assetId: null,
      workflowVersionId: null,
      recipeId: null,
    }));
    const emptyBucket = { totalCount: 0, items: [], hasMore: false };
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      structure: { ...aggregate().structure, unassignedShotCount: 500 },
      dailyProduction: {
        needsAttention: { totalCount: 500, items, hasMore: true },
        ready: emptyBucket,
        running: emptyBucket,
        review: emptyBucket,
        completed: emptyBucket,
        topAction: null,
      },
    })} onNavigate={onNavigate} />);

    const board = screen.getByRole("region", { name: "生产行动板" });
    expect(board.querySelectorAll(".project-command-daily-item")).toHaveLength(20);
    await user.click(screen.getByRole("button", { name: "查看全部 500" }));
    expect(onNavigate).toHaveBeenCalledWith({
      destination: "shots",
      section: "creation",
      projectId: "project-1",
      collectionFilter: { kind: "shots", sceneId: "UNASSIGNED" },
    });
  });

  it("offers the project-level bulk import dry-run entry without changing navigation", async () => {
    const user = userEvent.setup();
    const onOpenImport = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate()} onOpenImport={onOpenImport} />);

    await user.click(screen.getByRole("button", { name: "批量导入预检" }));
    expect(onOpenImport).toHaveBeenCalledTimes(1);
  });

  it("always exposes project management when a project is already active", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView {...viewProps()} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "管理项目" }));
    expect(onNavigate).toHaveBeenCalledWith({ destination: "projects" });
  });

  it.each([
    ["IMAGE_REVIEW", { kind: "IMAGE_REVIEW", shotId: "shot-image" }, { destination: "shots", section: "review", shotId: "shot-image", actionKind: "IMAGE_REVIEW" }],
    ["VIDEO_REVIEW", { kind: "VIDEO_REVIEW", shotId: "shot-video" }, { destination: "shots", section: "review", shotId: "shot-video", actionKind: "VIDEO_REVIEW" }],
    ["ACTIVE_PRODUCTION", { kind: "ACTIVE_PRODUCTION", batchId: "batch-active" }, { destination: "shots", section: "production", batchId: "batch-active", actionKind: "ACTIVE_PRODUCTION" }],
    ["AUTO_RESUMABLE", { kind: "AUTO_RESUMABLE", batchId: "batch-resume" }, { destination: "shots", section: "production", batchId: "batch-resume", actionKind: "AUTO_RESUMABLE" }],
    ["MISSING_CONFIG", { kind: "MISSING_CONFIG", shotId: "shot-config" }, { destination: "shots", section: "creation", shotId: "shot-config", actionKind: "MISSING_CONFIG" }],
    ["COMFY_BLOCKED", { kind: "COMFY_BLOCKED" }, { destination: "settings", section: "settings", actionKind: "COMFY_BLOCKED" }],
    ["NO_SHOTS", { kind: "NO_SHOTS" }, { destination: "shots", section: "creation", actionKind: "NO_SHOTS" }],
    ["READY", { kind: "READY", shotId: "shot-ready" }, { destination: "shots", section: "production", shotId: "shot-ready", actionKind: "READY" }],
  ] as const)("routes %s to a precise navigation request without side effects", async (_kind, action, expected) => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({ recommendedAction: { ...aggregate().recommendedAction, shotId: null, batchId: null, ...action } })} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "继续工作" }));

    expect(onNavigate).toHaveBeenCalledWith(expected);
  });

  it("uses the aggregate first batch target when an action omits its deep-link ID", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      queue: { ...aggregate().queue, firstActiveBatchId: "batch-first" },
      recommendedAction: { ...aggregate().recommendedAction, kind: "ACTIVE_PRODUCTION", shotId: null, batchId: null },
    })} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "继续工作" }));

    expect(onNavigate).toHaveBeenCalledWith({ destination: "shots", section: "production", batchId: "batch-first", actionKind: "ACTIVE_PRODUCTION" });
  });

  it("carries exact running shot and task targets from the derived queue facts", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      shots: { ...aggregate().shots, generating: 1, firstGeneratingShotId: "shot-running" },
      queue: { ...aggregate().queue, activeItems: 1, firstActiveBatchId: "batch-running", firstActiveShotId: "shot-running", firstActiveTaskId: "task-running" },
      recommendedAction: { ...aggregate().recommendedAction, kind: "ACTIVE_PRODUCTION", shotId: null, batchId: null, taskId: null, assetId: null },
    })} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "继续工作" }));

    expect(onNavigate).toHaveBeenCalledWith({
      destination: "shots",
      section: "production",
      shotId: "shot-running",
      batchId: "batch-running",
      taskId: "task-running",
      actionKind: "ACTIVE_PRODUCTION",
    });
  });

  it("opens the exact selected deliverable for completed projects", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      shots: { ...aggregate().shots, videoReview: 0, completed: 2, firstCompletedShotId: "shot-complete", firstCompletedAssetId: "asset-complete" },
      recommendedAction: { ...aggregate().recommendedAction, kind: "COMPLETE", shotId: null, batchId: null, taskId: null, assetId: null },
    })} onNavigate={onNavigate} />);

    await user.click(screen.getByRole("button", { name: "继续工作" }));

    expect(onNavigate).toHaveBeenCalledWith({
      destination: "assets",
      section: "assets",
      shotId: "shot-complete",
      assetId: "asset-complete",
      actionKind: "COMPLETE",
    });
  });

  it("surfaces the authoritative blocked reason beside the continuation target", () => {
    const html = renderToStaticMarkup(<ProjectCommandCenterView project={project} aggregate={aggregate({
      issues: [{ id: "runtime:offline", severity: "ERROR", title: "运行环境离线", detail: "请连接 ComfyUI 后重新预检。", source: "runtime" }],
      recommendedAction: { ...aggregate().recommendedAction, kind: "COMFY_BLOCKED", shotId: null, batchId: null, taskId: null, assetId: null },
    })} onNavigate={vi.fn()} />);

    expect(html).toContain("定位：运行环境设置");
    expect(html).toContain("原因：请连接 ComfyUI 后重新预检。");
  });

  it("recomputes continuation from refreshed facts instead of retaining UI state", () => {
    const { rerender } = render(<ProjectCommandCenterView project={project} aggregate={aggregate({
      recommendedAction: { ...aggregate().recommendedAction, kind: "ACTIVE_PRODUCTION", shotId: "shot-running", batchId: "batch-running", taskId: "task-running" },
    })} onNavigate={vi.fn()} />);
    expect(screen.getByText("定位：生产队列")).toBeTruthy();

    rerender(<ProjectCommandCenterView project={project} aggregate={aggregate({
      shots: { ...aggregate().shots, videoReview: 0, completed: 2, firstCompletedShotId: "shot-complete", firstCompletedAssetId: "asset-complete" },
      recommendedAction: { ...aggregate().recommendedAction, kind: "COMPLETE", shotId: null, batchId: null, taskId: null, assetId: null },
    })} onNavigate={vi.fn()} />);

    expect(screen.getByText("定位：交付素材")).toBeTruthy();
    expect(screen.getByRole("button", { name: "继续工作" })).toBeTruthy();
  });

  it("recommends binding configuration only for consistency projects with profiles but no bindings", () => {
    const consistency = aggregate({
      consistency: { characterProfiles: 1, sceneProfiles: 0, propProfiles: 0, styleProfiles: 0, referenceSets: 0, shotProfileBindings: 0, shotReferenceSetBindings: 0, scopeProfileBindings: 0, scopeReferenceSetBindings: 0, consistencyInUse: true },
      recommendedAction: { kind: "READY", priority: 11, reasonCode: "READY", reason: "legacy backend recommendation" },
    });
    const { container } = render(<ProjectCommandCenterView project={project} aggregate={consistency} onNavigate={vi.fn()} />);
    expect(container.textContent).toContain("配置镜头一致性");
  });
});
