// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type {
  ProjectWorkflowBindingInput,
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigUpdateRequest,
  ProjectWorkflowConfigView,
} from "../../types/projectWorkflow";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ShotInputValues, ShotStage, ShotStageConfig, ShotView } from "../../types/shot";
import type { TaskView } from "../../types/task";
import { ShotWorkspace } from "./ShotWorkspace";

const tauriMocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
  listShots: vi.fn(),
  getShot: vi.fn(),
  listRecentAssets: vi.fn(),
  listPromptLibrary: vi.fn(),
  listReferenceAnchors: vi.fn(),
  listProductionStructure: vi.fn(),
  getProductionBatchRunbook: vi.fn(),
  listBatchWorkflowPresets: vi.fn(),
  listProductionQueues: vi.fn(),
  getProductionQueueOverview: vi.fn(),
  listProductionPackageBindings: vi.fn(),
  getProductionAdmissionStatus: vi.fn(),
  bulkSetShotStageConfig: vi.fn(),
  setShotStageConfig: vi.fn(),
  generateShot: vi.fn(),
  planShotBatch: vi.fn(),
  createShotBatch: vi.fn(),
  startProductionQueue: vi.fn(),
}));

const taskEvents = vi.hoisted(() => ({
  subscribeTaskUpdates: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...tauriMocks };
});

vi.mock("../../services/taskEvents", () => taskEvents);
vi.mock("./ProjectStructureTree", () => ({ ProjectStructureTree: () => <div aria-label="项目结构" /> }));
vi.mock("../production/ProductionPackageWorkspace", () => ({ ProductionPackageWorkspace: () => <div aria-label="生产包工作区" /> }));
vi.mock("../production/ProductionBatchRunbookPanel", () => ({ ProductionBatchRunbookPanel: () => <div aria-label="生产手册" /> }));
vi.mock("../production/MultiPackageProductionBoard", () => ({ MultiPackageProductionBoard: () => <div aria-label="多生产包看板" /> }));
vi.mock("../production/ProductionQueueDrawer", () => ({ ProductionQueueDrawer: () => null }));
vi.mock("../production/ProductionMonitor", () => ({ ProductionMonitor: () => <div aria-label="生产监控" /> }));

const PROJECT_ID = "project-dev080";
const SHOT_ID = "shot-dev080";
const TIMESTAMP = "2026-09-03T00:00:00Z";

function imageRecipe(
  workflowId: string,
  workflowVersionId: string,
  recipeId: string,
  name: string,
): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId,
    recipeId,
    name,
    category: "image",
    mode: "text_to_image",
    fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
    outputTypes: ["image"],
  };
}

const CUSTOM_IMAGE_A = imageRecipe(
  "wfl_dev080_custom_image",
  "wv_dev080_custom_image",
  "rcp_dev080_custom_image",
  "DEV-080 Custom Image A",
);
const CUSTOM_IMAGE_B = imageRecipe(
  "wfl_dev080_custom_image_b",
  "wv_dev080_custom_image_b",
  "rcp_dev080_custom_image_b",
  "DEV-080 Custom Image B",
);
const BUILTIN_IMAGE = imageRecipe(
  "wfl_kera2_t2i_local_v2",
  "wv_builtin_kera2",
  "rcp_builtin_kera2",
  "内置 Krea2（fallback 禁止）",
);
const CATALOG = [CUSTOM_IMAGE_A, CUSTOM_IMAGE_B, BUILTIN_IMAGE];

function binding(
  recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">,
  available = true,
): ProjectWorkflowBindingView {
  return {
    stage: "IMAGE",
    mode: "DEFAULT",
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    createdAt: TIMESTAMP,
    updatedAt: TIMESTAMP,
    available,
  };
}

function projectConfig(imageDefault?: ProjectWorkflowBindingView): ProjectWorkflowConfigView {
  return { projectId: PROJECT_ID, imageDefault, videoModeOverrides: [] };
}

function stageConfig(stage: ShotStage, recipe: RecipeViewModel): ShotStageConfig {
  return {
    stage,
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    scalarValues: {},
    updatedAt: TIMESTAMP,
  };
}

function shot(initialStageConfigs: ShotStageConfig[] = []): ShotView {
  return {
    id: SHOT_ID,
    projectId: PROJECT_ID,
    ordinal: 0,
    name: "DEV-080 test shot",
    promptText: "A custom workflow reaches formal production.",
    createdAt: TIMESTAMP,
    updatedAt: TIMESTAMP,
    status: "DRAFT",
    imageStatus: "DRAFT",
    videoStatus: "DRAFT",
    stageConfigs: initialStageConfigs,
    referenceAssets: [],
    generationLinks: [],
  };
}

interface FakeComfySubmission {
  batchId: string;
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  workflowJson: { workflowId: string; workflowVersionId: string; recipeId: string };
}

/** SQLite-shaped state plus a fake Comfy boundary; no real process or GPU is needed. */
class FakeSqliteProductionAdapter {
  private config: ProjectWorkflowConfigView;
  private readonly shots = new Map<string, ShotView>();
  private readonly batches = new Map<string, ProductionBatchDetail>();
  private readonly catalog: RecipeViewModel[];
  private readonly defaultAfterBatch: RecipeViewModel;
  readonly events: string[] = [];
  readonly comfySubmissions: FakeComfySubmission[] = [];

  constructor(
    initialConfig: ProjectWorkflowConfigView,
    initialShots: ShotView[],
    catalog: RecipeViewModel[] = CATALOG,
    defaultAfterBatch: RecipeViewModel = CUSTOM_IMAGE_B,
  ) {
    this.config = initialConfig;
    this.catalog = catalog;
    this.defaultAfterBatch = defaultAfterBatch;
    for (const item of initialShots) this.shots.set(item.id, item);
  }

  async getProjectWorkflowConfig(projectId: string): Promise<ProjectWorkflowConfigView> {
    expect(projectId).toBe(PROJECT_ID);
    return this.config;
  }

  async replaceProjectWorkflowConfig(projectId: string, request: ProjectWorkflowConfigUpdateRequest): Promise<ProjectWorkflowConfigView> {
    expect(projectId).toBe(PROJECT_ID);
    const view = (input: ProjectWorkflowBindingInput): ProjectWorkflowBindingView => ({
      ...input,
      createdAt: TIMESTAMP,
      updatedAt: TIMESTAMP,
      available: true,
    });
    const imageDefault = request.bindings.find((item) => item.stage === "IMAGE" && item.mode === "DEFAULT");
    const videoDefault = request.bindings.find((item) => item.stage === "VIDEO" && item.mode === "DEFAULT");
    this.config = {
      ...this.config,
      imageDefault: imageDefault ? view(imageDefault) : undefined,
      videoDefault: videoDefault ? view(videoDefault) : undefined,
      videoModeOverrides: request.bindings.filter((item) => item.stage === "VIDEO" && item.mode !== "DEFAULT").map(view),
    };
    return this.config;
  }

  async listShots(projectId: string): Promise<ShotView[]> {
    expect(projectId).toBe(PROJECT_ID);
    return [...this.shots.values()].map((item) => this.cloneShot(item));
  }

  async getShot(projectId: string, shotId: string): Promise<ShotView> {
    expect(projectId).toBe(PROJECT_ID);
    const item = this.shots.get(shotId);
    if (!item) throw new Error(`missing shot ${shotId}`);
    return this.cloneShot(item);
  }

  async setShotStageConfig(request: {
    projectId: string;
    shotId: string;
    stage: ShotStage;
    workflowVersionId: string;
    recipeId: string;
    values: ShotInputValues;
  }): Promise<ShotView> {
    expect(request.projectId).toBe(PROJECT_ID);
    const current = this.shots.get(request.shotId);
    if (!current) throw new Error(`missing shot ${request.shotId}`);
    const nextConfig = stageConfigFromRequest(request);
    this.shots.set(request.shotId, {
      ...current,
      stageConfigs: [...current.stageConfigs.filter((item) => item.stage !== request.stage), nextConfig],
      updatedAt: TIMESTAMP,
    });
    this.events.push(`set:${request.stage}:${request.workflowVersionId}:${request.recipeId}`);
    return this.cloneShot(this.shots.get(request.shotId)!);
  }

  async bulkSetShotStageConfig(request: {
    projectId: string;
    stage: ShotStage;
    shotIds: string[];
    workflowVersionId: string;
    recipeId: string;
    values: ShotInputValues;
  }): Promise<{ projectId: string; stage: ShotStage; configuredShotIds: string[]; promptUpdatedShotIds: string[] }> {
    for (const shotId of request.shotIds) {
      await this.setShotStageConfig({ ...request, shotId });
    }
    return { projectId: request.projectId, stage: request.stage, configuredShotIds: request.shotIds, promptUpdatedShotIds: [] };
  }

  async generateShot(request: { projectId: string; shotId: string; stage: ShotStage; values?: ShotInputValues }): Promise<TaskView> {
    const current = this.shots.get(request.shotId);
    const config = current?.stageConfigs.find((item) => item.stage === request.stage);
    if (!config) throw new Error("stage config missing");
    this.events.push(`generate:${request.stage}:${config.workflowVersionId}:${config.recipeId}`);
    return {
      id: "task-dev080",
      projectId: request.projectId,
      status: "QUEUED",
      progress: { mode: "indeterminate" },
      createdAt: TIMESTAMP,
      outputAssetIds: [],
    };
  }

  async planShotBatch(projectId: string, stage: ShotStage) {
    const rows = [...this.shots.values()].map((item) => {
      const config = item.stageConfigs.find((candidate) => candidate.stage === stage);
      return {
        shotId: item.id,
        ordinal: item.ordinal,
        name: item.name,
        stage,
        workflowVersionId: config?.workflowVersionId,
        recipeId: config?.recipeId,
        currentStatus: "READY",
        referenceCount: 0,
        eligible: Boolean(config),
        blockingReasons: config ? [] : ["未配置阶段工作流"],
      };
    });
    return {
      projectId,
      stage,
      maxItems: 50,
      eligibleCount: rows.filter((item) => item.eligible).length,
      rows,
    };
  }

  async createShotBatch(request: { projectId: string; stage: ShotStage; shotIds: string[] }): Promise<ProductionBatchDetail> {
    const items = request.shotIds.map((shotId, ordinal) => {
      const config = this.shots.get(shotId)?.stageConfigs.find((item) => item.stage === request.stage);
      if (!config) throw new Error(`missing frozen config ${shotId}`);
      return {
        id: `pbi-dev080-${ordinal}`,
        ordinal,
        workflowVersionId: config.workflowVersionId,
        recipeId: config.recipeId,
        status: "PENDING" as const,
      };
    });
    const detail: ProductionBatchDetail = {
      id: "pbt-dev080",
      projectId: request.projectId,
      name: "DEV-080 frozen batch",
      status: "READY",
      continueOnFailure: false,
      createdAt: TIMESTAMP,
      updatedAt: TIMESTAMP,
      total: items.length,
      pending: items.length,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      skipped: 0,
      items,
    };
    this.batches.set(detail.id, detail);
    return detail;
  }

  async startProductionQueue(projectId: string, batchId: string): Promise<ProductionBatchDetail> {
    const batch = this.batches.get(batchId);
    if (!batch || batch.projectId !== projectId) throw new Error("missing batch");
    this.config = { ...this.config, imageDefault: binding(this.defaultAfterBatch) };
    for (const item of batch.items) {
      const recipe = this.catalog.find((candidate) => candidate.workflowVersionId === item.workflowVersionId && candidate.recipeId === item.recipeId);
      if (!recipe) throw new Error("frozen recipe missing from catalog");
      this.comfySubmissions.push({
        batchId,
        workflowId: recipe.workflowId,
        workflowVersionId: item.workflowVersionId,
        recipeId: item.recipeId,
        workflowJson: {
          workflowId: recipe.workflowId,
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
        },
      });
      this.events.push(`execute:${item.workflowVersionId}:${item.recipeId}`);
    }
    const running = { ...batch, status: "RUNNING" as const, pending: 0, running: batch.items.length };
    this.batches.set(batchId, running);
    return running;
  }

  frozenBatch(batchId: string): ProductionBatchDetail | undefined {
    return this.batches.get(batchId);
  }

  queueSummaries(): ProductionBatchSummary[] {
    return [...this.batches.values()].map(({ id, projectId, name, status, continueOnFailure, createdAt, updatedAt }) => ({
      id,
      projectId,
      name,
      status,
      continueOnFailure,
      createdAt,
      updatedAt,
    }));
  }

  queueOverview(): ProductionQueueOverview {
    const queues = this.queueSummaries();
    return {
      totalQueues: queues.length,
      runningQueues: queues.filter((item) => item.status === "RUNNING").length,
      pausedQueues: 0,
      completedQueues: queues.filter((item) => item.status === "COMPLETED").length,
      archivedQueues: 0,
      totalItems: [...this.batches.values()].reduce((total, item) => total + item.total, 0),
      pendingItems: [...this.batches.values()].reduce((total, item) => total + item.pending, 0),
      activeItems: [...this.batches.values()].reduce((total, item) => total + item.running, 0),
      succeededItems: 0,
      failedItems: 0,
      cancelledItems: 0,
      skippedItems: 0,
    };
  }

  private cloneShot(item: ShotView): ShotView {
    return { ...item, stageConfigs: item.stageConfigs.map((config) => ({ ...config, scalarValues: { ...config.scalarValues } })) };
  }
}

function stageConfigFromRequest(request: {
  stage: ShotStage;
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
}): ShotStageConfig {
  return {
    stage: request.stage,
    workflowVersionId: request.workflowVersionId,
    recipeId: request.recipeId,
    scalarValues: Object.fromEntries(
      Object.entries(request.values).filter(([, value]) => value.type !== "string"),
    ) as ShotStageConfig["scalarValues"],
    updatedAt: TIMESTAMP,
  };
}

function installAdapter(adapter: FakeSqliteProductionAdapter): void {
  tauriMocks.getProjectWorkflowConfig.mockImplementation((projectId: string) => adapter.getProjectWorkflowConfig(projectId));
  tauriMocks.replaceProjectWorkflowConfig.mockImplementation((projectId: string, request: ProjectWorkflowConfigUpdateRequest) => adapter.replaceProjectWorkflowConfig(projectId, request));
  tauriMocks.listShots.mockImplementation((projectId: string) => adapter.listShots(projectId));
  tauriMocks.getShot.mockImplementation((projectId: string, shotId: string) => adapter.getShot(projectId, shotId));
  tauriMocks.bulkSetShotStageConfig.mockImplementation((request) => adapter.bulkSetShotStageConfig(request));
  tauriMocks.setShotStageConfig.mockImplementation((request) => adapter.setShotStageConfig(request));
  tauriMocks.generateShot.mockImplementation((request) => adapter.generateShot(request));
  tauriMocks.planShotBatch.mockImplementation((projectId: string, stage: ShotStage) => adapter.planShotBatch(projectId, stage));
  tauriMocks.createShotBatch.mockImplementation((request) => adapter.createShotBatch(request));
  tauriMocks.startProductionQueue.mockImplementation((projectId: string, batchId: string) => adapter.startProductionQueue(projectId, batchId));
  tauriMocks.listProductionQueues.mockImplementation(() => adapter.queueSummaries());
  tauriMocks.getProductionQueueOverview.mockImplementation(() => adapter.queueOverview());
  tauriMocks.listRecentAssets.mockResolvedValue([]);
  tauriMocks.listPromptLibrary.mockResolvedValue({ items: [] });
  tauriMocks.listReferenceAnchors.mockResolvedValue([]);
  tauriMocks.listProductionStructure.mockResolvedValue({ projectId: PROJECT_ID, series: [], unassignedShotIds: [] });
  tauriMocks.getProductionBatchRunbook.mockResolvedValue({ projectId: PROJECT_ID, rows: [] });
  tauriMocks.listBatchWorkflowPresets.mockResolvedValue([]);
  tauriMocks.listProductionPackageBindings.mockResolvedValue([]);
  tauriMocks.getProductionAdmissionStatus.mockResolvedValue({ busy: false });
}

function renderWorkspace(adapter: FakeSqliteProductionAdapter, mode: "creation" | "production" = "creation"): void {
  installAdapter(adapter);
  render(
    <ShotWorkspace
      projectId={PROJECT_ID}
      projectName="DEV-080 Project"
      catalog={CATALOG}
      initialSelectedShotId={SHOT_ID}
      mode={mode}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  taskEvents.subscribeTaskUpdates.mockReset().mockResolvedValue(() => undefined);
});

afterEach(cleanup);

describe("DEV-080 formal project workflow production UAT", () => {
  it("carries a custom image default into Shot config, freezes the exact pair, and reaches fake Comfy", async () => {
    const adapter = new FakeSqliteProductionAdapter(projectConfig(binding(CUSTOM_IMAGE_A)), [shot()]);
    const user = userEvent.setup();
    renderWorkspace(adapter, "production");

    await user.click(await screen.findByRole("tab", { name: "项目生产" }));
    await user.click(screen.getByRole("button", { name: "全选" }));
    await user.click(screen.getByRole("button", { name: "配置图片阶段" }));

    await waitFor(() => expect(tauriMocks.bulkSetShotStageConfig).toHaveBeenCalledWith(expect.objectContaining({
      projectId: PROJECT_ID,
      stage: "image",
      workflowVersionId: CUSTOM_IMAGE_A.workflowVersionId,
      recipeId: CUSTOM_IMAGE_A.recipeId,
    })));

    await user.click(screen.getByRole("button", { name: "创建图片批次" }));
    await waitFor(() => expect(adapter.comfySubmissions).toHaveLength(1));

    const frozen = adapter.frozenBatch("pbt-dev080");
    expect(frozen?.items[0]).toMatchObject({
      workflowVersionId: CUSTOM_IMAGE_A.workflowVersionId,
      recipeId: CUSTOM_IMAGE_A.recipeId,
    });
    expect(adapter.comfySubmissions[0]).toMatchObject({
      workflowId: CUSTOM_IMAGE_A.workflowId,
      workflowVersionId: CUSTOM_IMAGE_A.workflowVersionId,
      recipeId: CUSTOM_IMAGE_A.recipeId,
      workflowJson: {
        workflowId: CUSTOM_IMAGE_A.workflowId,
        workflowVersionId: CUSTOM_IMAGE_A.workflowVersionId,
        recipeId: CUSTOM_IMAGE_A.recipeId,
      },
    });
    expect((await adapter.getProjectWorkflowConfig(PROJECT_ID)).imageDefault?.recipeId).toBe(CUSTOM_IMAGE_B.recipeId);
    expect(frozen?.items[0].recipeId).toBe(CUSTOM_IMAGE_A.recipeId);
  });

  it("gives a persisted Shot stage config precedence over the project default", async () => {
    const adapter = new FakeSqliteProductionAdapter(
      projectConfig(binding(CUSTOM_IMAGE_A)),
      [shot([stageConfig("image", CUSTOM_IMAGE_B)])],
    );
    renderWorkspace(adapter);

    const select = await screen.findByLabelText("工作流 / 配方") as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe(CUSTOM_IMAGE_B.recipeId));
    expect(select.value).not.toBe(CUSTOM_IMAGE_A.recipeId);
  });

  it("fails closed when a project binding is unavailable instead of selecting built-in Krea2", async () => {
    const adapter = new FakeSqliteProductionAdapter(projectConfig(binding(CUSTOM_IMAGE_A, false)), [shot()]);
    renderWorkspace(adapter);

    const select = await screen.findByLabelText("工作流 / 配方") as HTMLSelectElement;
    await waitFor(() => expect(select.value).not.toBe(BUILTIN_IMAGE.recipeId));
    expect(await screen.findByRole("alert")).toBeTruthy();
  });

  it("persists a manual Shot workflow before direct generation", async () => {
    const adapter = new FakeSqliteProductionAdapter(
      projectConfig(binding(CUSTOM_IMAGE_A)),
      [shot([stageConfig("image", CUSTOM_IMAGE_A)])],
    );
    const user = userEvent.setup();
    renderWorkspace(adapter);

    const select = await screen.findByLabelText("工作流 / 配方");
    await user.selectOptions(select, CUSTOM_IMAGE_B.recipeId);
    await user.click(screen.getByRole("button", { name: "生成" }));

    await waitFor(() => expect(adapter.events.slice(-2)).toEqual([
      `set:image:${CUSTOM_IMAGE_B.workflowVersionId}:${CUSTOM_IMAGE_B.recipeId}`,
      `generate:image:${CUSTOM_IMAGE_B.workflowVersionId}:${CUSTOM_IMAGE_B.recipeId}`,
    ]));
  });
});
