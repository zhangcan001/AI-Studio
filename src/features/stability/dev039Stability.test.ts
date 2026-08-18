import { beforeEach, describe, expect, it } from "vitest";
import type { PromptEntryView } from "../../types/prompt";
import type { ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProjectView } from "../../types/project";
import type { ShotView } from "../../types/shot";
import type { TaskHistoryItem } from "../../types/history";
import type { TaskView } from "../../types/task";
import {
  DEFAULT_PROJECT_ID,
  resolveActiveProjectId,
  useProjectStore,
} from "../../stores/projectStore";
import { useTaskStore } from "../../stores/taskStore";
import {
  recentProductionQueues,
  recentPromptEntries,
  recentWorkflowRecords,
  summarizeProductionQueues,
} from "../production/productionUx";
import {
  readStoredProductionQueueId,
  rememberProductionQueue,
  selectProductionQueueId,
} from "../studio/productionQueueSelection";
import {
  buildShotListView,
  defaultShotListControls,
} from "../shots/shotListQuery";

const NOW = "2026-08-18T00:00:00Z";

const project = (id: string, name = id): ProjectView => ({
  id,
  name,
  description: null,
  createdAt: NOW,
  updatedAt: NOW,
});

const projects = [project(DEFAULT_PROJECT_ID, "Default"), project("prj_other", "Other")];

const task = (id: string, projectId: string): TaskView => ({
  id,
  projectId,
  status: "SUCCEEDED",
  progress: { mode: "indeterminate" },
  createdAt: NOW,
  finishedAt: NOW,
  outputAssetIds: [],
});

const shot = (id: string, ordinal: number, projectId = "prj_large"): ShotView => ({
  id,
  projectId,
  ordinal,
  name: `Shot ${ordinal + 1}`,
  promptText: `Prompt ${ordinal + 1}`,
  createdAt: NOW,
  updatedAt: NOW,
  status: "DRAFT",
  imageStatus: "DRAFT",
  videoStatus: "DRAFT",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
});

const queue = (id: string, updatedAt: string, status: ProductionBatchSummary["status"]): ProductionBatchSummary => ({
  id,
  projectId: "prj_large",
  name: id,
  status,
  continueOnFailure: true,
  createdAt: NOW,
  updatedAt,
});

const prompt = (id: string, updatedAt: string): PromptEntryView => ({
  id,
  projectId: "prj_large",
  kind: "prompt",
  name: id,
  tags: [],
  createdAt: NOW,
  updatedAt,
  versionCount: 1,
  versions: [{ id: `${id}-v1`, promptId: id, version: 1, text: id, createdAt: NOW }],
});

const historyItem = (id: string, index: number): TaskHistoryItem => ({
  id,
  workflowId: "wfl_krea2_t2i_local_v2",
  workflowVersionId: "wfv_krea2",
  recipeId: "rcp_krea2",
  workflowName: "Krea2",
  status: "SUCCEEDED",
  createdAt: NOW,
  finishedAt: `2026-08-18T00:${String(index % 60).padStart(2, "0")}:00Z`,
  outputCount: 1,
});

interface CommandCenterSnapshot {
  queues: ProductionBatchSummary[];
  overview: ProductionQueueOverview;
  history: TaskHistoryItem[];
  prompts: PromptEntryView[];
}

interface CommandCenterSource {
  calls: Record<keyof CommandCenterSnapshot, number>;
  reload: () => Promise<{
    summary: ReturnType<typeof summarizeProductionQueues>;
    recentQueueIds: string[];
    recentWorkflowIds: string[];
    recentPromptIds: string[];
  }>;
}

function createCommandCenterSource(): CommandCenterSource {
  const snapshot: CommandCenterSnapshot = {
    queues: [
      queue("batch-running", "2026-08-18T00:03:00Z", "RUNNING"),
      queue("batch-ready", "2026-08-18T00:02:00Z", "READY"),
      queue("batch-completed", "2026-08-18T00:01:00Z", "COMPLETED"),
    ],
    overview: {
      totalQueues: 3,
      runningQueues: 1,
      pausedQueues: 0,
      completedQueues: 1,
      archivedQueues: 0,
      totalItems: 500,
      pendingItems: 12,
      activeItems: 4,
      succeededItems: 484,
      failedItems: 0,
      cancelledItems: 0,
      skippedItems: 0,
    },
    history: Array.from({ length: 500 }, (_, index) => historyItem(`task-${index + 1}`, index)),
    prompts: Array.from({ length: 30 }, (_, index) => prompt(`prompt-${index + 1}`, `2026-08-18T00:${String(index).padStart(2, "0")}:00Z`)),
  };
  const calls = {
    queues: 0,
    overview: 0,
    history: 0,
    prompts: 0,
  };

  return {
    calls,
    async reload() {
      const [queues, overview, history, prompts] = await Promise.all([
        Promise.resolve().then(() => { calls.queues += 1; return snapshot.queues; }),
        Promise.resolve().then(() => { calls.overview += 1; return snapshot.overview; }),
        Promise.resolve().then(() => { calls.history += 1; return snapshot.history.slice(0, 50); }),
        Promise.resolve().then(() => { calls.prompts += 1; return snapshot.prompts; }),
      ]);

      return {
        summary: summarizeProductionQueues(queues, overview),
        recentQueueIds: recentProductionQueues(queues, 5).map((item) => item.id),
        recentWorkflowIds: recentWorkflowRecords(history.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          workflowName: item.workflowName,
          lastUsedAt: item.finishedAt ?? item.createdAt,
        })), 5).map((item) => `${item.workflowVersionId}:${item.recipeId}`),
        recentPromptIds: recentPromptEntries(prompts, 5).map((item) => item.id),
      };
    },
  };
}

function resetFrontendStores() {
  useProjectStore.setState({ projects: [], activeProjectId: undefined, loading: true, error: undefined });
  useTaskStore.getState().clear();
}

function installStorage() {
  const values = new Map<string, string>();
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
      clear: () => values.clear(),
    },
  });
}

describe("DEV-039 no-GPU frontend stability harness", () => {
  beforeEach(() => {
    installStorage();
    resetFrontendStores();
  });

  it("keeps a fresh project open/close session clean for 20 cycles", () => {
    for (let cycle = 0; cycle < 20; cycle += 1) {
      resetFrontendStores();
      useProjectStore.getState().setProjects(projects);
      expect(useProjectStore.getState().activeProjectId).toBe(DEFAULT_PROJECT_ID);
      expect(useProjectStore.getState().activeProject()?.id).toBe(DEFAULT_PROJECT_ID);

      useTaskStore.getState().upsertTask(task(`task-${cycle}`, DEFAULT_PROJECT_ID));
      expect(useTaskStore.getState().recentTasks).toHaveLength(1);

      resetFrontendStores();
      expect(useProjectStore.getState().activeProjectId).toBeUndefined();
      expect(useTaskStore.getState().recentTasks).toEqual([]);
      expect(useTaskStore.getState().currentTask).toBeUndefined();
    }
  });

  it("switches project workspace context for 20 cycles without task leakage", () => {
    useProjectStore.getState().setProjects(projects);
    for (let cycle = 0; cycle < 20; cycle += 1) {
      const projectId = cycle % 2 === 0 ? DEFAULT_PROJECT_ID : "prj_other";
      useTaskStore.getState().clear();
      useProjectStore.getState().setActiveProject(projectId);
      useTaskStore.getState().setRecentTasks([task(`task-${cycle}`, projectId)]);

      expect(useProjectStore.getState().activeProject()?.id).toBe(projectId);
      expect(useTaskStore.getState().currentTask?.projectId).toBe(projectId);
      expect(useTaskStore.getState().recentTasks.every((item) => item.projectId === projectId)).toBe(true);
    }
  });

  it("reloads a 500-shot project selection state for 100 cycles", () => {
    const shots = Array.from({ length: 500 }, (_, index) => shot(`shot-${index + 1}`, index));
    let selectedShotId: string | undefined = "shot-250";
    for (let cycle = 0; cycle < 100; cycle += 1) {
      const result = buildShotListView(shots, defaultShotListControls());
      selectedShotId = selectedShotId && result.filteredShots.some((item) => item.id === selectedShotId)
        ? selectedShotId
        : result.filteredShots[0]?.id;
      expect(result.filteredCount).toBe(500);
      expect(result.pageShots).toHaveLength(50);
      expect(selectedShotId).toBe("shot-250");
    }
  });

  it("reloads the 500-shot Command Center data surface for 30 cycles", async () => {
    const source = createCommandCenterSource();
    let last;
    for (let cycle = 0; cycle < 30; cycle += 1) last = await source.reload();

    expect(last?.summary).toMatchObject({ queueCount: 3, runningCount: 1, activeItemCount: 4 });
    expect(last?.recentQueueIds).toEqual(["batch-running", "batch-ready", "batch-completed"]);
    expect(last?.recentWorkflowIds).toEqual(["wfv_krea2:rcp_krea2"]);
    expect(last?.recentPromptIds).toHaveLength(5);
    expect(source.calls).toEqual({ queues: 30, overview: 30, history: 30, prompts: 30 });
  });

  it("refreshes Command Center summaries 20 cycles without duplicate recent records", async () => {
    const source = createCommandCenterSource();
    for (let cycle = 0; cycle < 20; cycle += 1) {
      const result = await source.reload();
      expect(new Set(result.recentQueueIds).size).toBe(result.recentQueueIds.length);
      expect(new Set(result.recentWorkflowIds).size).toBe(result.recentWorkflowIds.length);
      expect(new Set(result.recentPromptIds).size).toBe(result.recentPromptIds.length);
      expect(result.summary.latestQueueId).toBe("batch-running");
    }

    expect(source.calls).toEqual({ queues: 20, overview: 20, history: 20, prompts: 20 });
  });

  it("resumes persisted settings, project, and queue context after 20 restarts", () => {
    const persistedSettings = { endpoint: "http://127.0.0.1:8188", schemaVersion: 1 };
    const settingsStorageKey = "aistudio.dev039.settings";
    globalThis.localStorage.setItem(settingsStorageKey, JSON.stringify(persistedSettings));
    useProjectStore.getState().setProjects(projects);
    useProjectStore.getState().setActiveProject("prj_other");
    rememberProductionQueue("prj_other", "batch-running");

    for (let cycle = 0; cycle < 20; cycle += 1) {
      resetFrontendStores();
      const restoredSettings = JSON.parse(globalThis.localStorage.getItem(settingsStorageKey) ?? "null") as typeof persistedSettings;
      useProjectStore.getState().setProjects(projects);
      const restoredProjectId = resolveActiveProjectId(projects, "prj_other");
      useProjectStore.getState().setActiveProject(restoredProjectId ?? DEFAULT_PROJECT_ID);
      const restoredQueueId = selectProductionQueueId(
        [{
          ...queue("batch-running", "2026-08-18T00:03:00Z", "RUNNING"),
        }],
        [readStoredProductionQueueId("prj_other")],
        true,
      );

      expect(restoredSettings).toEqual(persistedSettings);
      expect(useProjectStore.getState().activeProjectId).toBe("prj_other");
      expect(restoredQueueId).toBe("batch-running");
    }
  });

  it("falls back safely when the selected shot or project was deleted", () => {
    const shots = Array.from({ length: 500 }, (_, index) => shot(`shot-${index + 1}`, index));
    const remainingShots = shots.filter((item) => item.id !== "shot-250");
    let selectedShotId: string | undefined = "shot-250";
    for (let cycle = 0; cycle < 20; cycle += 1) {
      selectedShotId = remainingShots.find((item) => item.id === selectedShotId)?.id ?? remainingShots[0]?.id;
      expect(selectedShotId).toBe("shot-1");
      expect(resolveActiveProjectId([project("prj_other")], "deleted-project")).toBe("prj_other");
      expect(selectProductionQueueId([], ["deleted-batch"], true)).toBeUndefined();
    }
  });
});
