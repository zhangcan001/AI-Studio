import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import * as tauriClient from "../../services/tauriClient";
import type {
  ScenePreparationView,
  ShotProductionPlanSummary,
} from "../../types/productionPreparation";
import {
  MAX_PREPARATION_BATCH_ITEMS,
  preparationCanSelect,
} from "../../types/productionPreparation";
import {
  SceneProductionPreparation,
  preparationSelectionLimit,
} from "./SceneProductionPreparation";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const summary = (overrides: Partial<ShotProductionPlanSummary> = {}): ShotProductionPlanSummary => ({
  shotId: "shot-1",
  ordinal: 0,
  name: "雨夜入口",
  status: "READY",
  score: 95,
  warningCount: 1,
  incompleteCount: 0,
  blockerCount: 0,
  contextHash: "hash-shot-1",
  characterNames: ["主角"],
  characterCount: 1,
  sceneProfileName: "雨夜入口",
  referenceCount: 2,
  workflowVersionId: "workflow-1",
  recipeId: "recipe-1",
  currentStageStatus: "未开始",
  alreadyPrepared: false,
  existingBatchIds: [],
  matchingPreparedBatchIds: [],
  stalePreparedBatchIds: [],
  blockers: [],
  warnings: [],
  legacy: false,
  ...overrides,
});

const view: ScenePreparationView = {
  projectId: "project-1",
  sceneId: "scene-1",
  sceneName: "雨夜入口",
  stage: "image",
  total: 4,
  readyCount: 2,
  incompleteCount: 1,
  blockedCount: 1,
  preparedCount: 1,
  warningCount: 1,
  evaluatedAt: "2026-08-26T08:00:00Z",
  items: [
    summary(),
    summary({ shotId: "shot-2", ordinal: 1, name: "巷口近景", alreadyPrepared: true, matchingPreparedBatchIds: ["batch-1"], stalePreparedBatchIds: ["batch-old"] }),
    summary({ shotId: "shot-3", ordinal: 2, name: "屋檐切换", status: "INCOMPLETE", score: 70, warningCount: 0, incompleteCount: 1, blockerCount: 0, blockers: ["缺少视频关键帧"] }),
    summary({ shotId: "shot-4", ordinal: 3, name: "远景收束", status: "BLOCKED", score: 40, warningCount: 0, incompleteCount: 0, blockerCount: 1, blockers: ["ComfyUI 离线"] }),
  ],
};

describe("SceneProductionPreparation", () => {
  it("renders the scene preparation first screen with counts and only READY selection affordances", () => {
    const html = renderToStaticMarkup(
      <SceneProductionPreparation
        projectId="project-1"
        sceneOptions={[{ value: "scene-1", label: "S01 / 雨夜入口" }]}
        currentSceneId="scene-1"
        initialView={view}
      />,
    );

    expect(html).toContain("场景生产准备");
    expect(html).toContain("<span>总镜头</span><strong>4</strong>");
    expect(html).toContain("<span>READY</span><strong>2</strong>");
    expect(html).toContain("<span>INCOMPLETE</span><strong>1</strong>");
    expect(html).toContain("<span>BLOCKED</span><strong>1</strong>");
    expect(html).toContain("<span>已准备</span><strong>1</strong>");
    expect(html).toContain("READY");
    expect(html).toContain("INCOMPLETE");
    expect(html).toContain("BLOCKED");
    expect(html).toContain("已准备");
    expect(html).toContain("选择全部 READY");
    expect(html).toContain("已有旧上下文准备版本");
    expect(html).toContain("ComfyUI 离线");
    expect(html).not.toContain("立即启动");
    expect(html).not.toContain("开始全部");
    expect(html).not.toContain("启动生产");

    const checkboxes = html.match(/<input[^>]*type="checkbox"[^>]*>/g) ?? [];
    expect(checkboxes).toHaveLength(4);
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 雨夜入口"'))).not.toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 巷口近景"'))).toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 屋檐切换"'))).toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 远景收束"'))).toContain("disabled");
  });

  it("renders the 100-shot selection boundary and excludes incomplete, blocked, and already-prepared shots", () => {
    expect(MAX_PREPARATION_BATCH_ITEMS).toBe(100);
    expect(preparationSelectionLimit(500)).toBe(100);
    expect(preparationCanSelect(summary())).toBe(true);
    expect(preparationCanSelect(summary({ status: "INCOMPLETE" }))).toBe(false);
    expect(preparationCanSelect(summary({ status: "BLOCKED" }))).toBe(false);
    expect(preparationCanSelect(summary({ alreadyPrepared: true }))).toBe(false);

    const html = renderToStaticMarkup(
      <SceneProductionPreparation
        projectId="project-1"
        sceneOptions={[{ value: "scene-1", label: "S01 / 雨夜入口" }]}
        currentSceneId="scene-1"
        initialView={largeReadyView}
      />,
    );
    expect(html).toContain("READY 镜头超过 100 个");
    expect(html).toContain("只取前 100 个");
    expect(html.match(/type="checkbox"/g) ?? []).toHaveLength(105);
  });
});

describe("Scene preparation client boundary", () => {
  it("uses one preflight envelope and admission never calls startProductionQueue", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(view);
    await tauriClient.getSceneProductionPreflight({ projectId: "project-1", sceneId: "scene-1", stage: "image" });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_preflight", {
      request: { projectId: "project-1", sceneId: "scene-1", stage: "image" },
    });

    const startSpy = vi.spyOn(tauriClient, "startProductionQueue");
    vi.mocked(invoke).mockResolvedValueOnce({
      projectId: "project-1",
      stage: "image",
      requestedCount: 1,
      createdCount: 1,
      skippedIncomplete: 0,
      skippedBlocked: 0,
      alreadyPreparedCount: 0,
      createdBatchIds: ["batch-1"],
      matchingPreparedBatchIds: [],
    });
    await tauriClient.admitSceneProduction({
      projectId: "project-1",
      sceneId: "scene-1",
      stage: "image",
      shotIds: ["shot-1"],
      allowPartial: false,
    });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_admit", {
      request: {
        projectId: "project-1",
        sceneId: "scene-1",
        stage: "image",
        shotIds: ["shot-1"],
        allowPartial: false,
      },
    });
    expect(startSpy).not.toHaveBeenCalled();
    startSpy.mockRestore();
  });

  it("clicks Select all READY at 100, admits the selection, and navigates to Queue without starting it", async () => {
    const admissionResult = {
      projectId: "project-1",
      sceneId: "scene-1",
      stage: "image" as const,
      batchId: "batch-100",
      createdBatchIds: ["batch-100"],
      createdCount: 100,
      skippedIncomplete: 0,
      skippedBlocked: 0,
      alreadyPreparedCount: 0,
      matchingPreparedBatchIds: [],
    };
    const onOpenProductionQueue = vi.fn();
    const hookHarness = new HookHarness();
    const admitMock = vi.fn().mockResolvedValue(admissionResult);
    const startMock = vi.fn();

    vi.resetModules();
    vi.doMock("react", async () => {
      const actual = await vi.importActual<typeof import("react")>("react");
      return {
        ...actual,
        useState: hookHarness.useState.bind(hookHarness),
        useMemo: hookHarness.useMemo.bind(hookHarness),
        useEffect: hookHarness.useEffect.bind(hookHarness),
      };
    });
    vi.doMock("../../services/tauriClient", async () => {
      const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
      return {
        ...actual,
        getSceneProductionPreflight: vi.fn().mockResolvedValue(largeReadyView),
        getShotProductionPlanDetail: vi.fn(),
        admitSceneProduction: admitMock,
        startProductionQueue: startMock,
      };
    });

    try {
      const { SceneProductionPreparation: Preparation } = await import("./SceneProductionPreparation");
      const props = {
        projectId: "project-1",
        sceneOptions: [{ value: "scene-1", label: "S01 / 雨夜入口" }],
        currentSceneId: "scene-1",
        initialView: largeReadyView,
        onOpenProductionQueue,
      };
      const render = () => {
        hookHarness.beginRender();
        return materialize(Preparation(props));
      };

      let tree = render();
      const selectAll = findSingle(tree, (element) => element.type === "button" && textContent(element) === "选择全部 READY");
      (selectAll.props.onClick as () => void)();
      tree = render();
      expect(findElements(tree, (element) => element.type === "input" && element.props.type === "checkbox" && element.props.checked === true)).toHaveLength(100);

      const admit = findSingle(tree, (element) => element.type === "button" && textContent(element) === "加入生产");
      (admit.props.onClick as () => void)();
      await flushMicrotasks();
      tree = render();

      expect(admitMock).toHaveBeenCalledWith({
        projectId: "project-1",
        sceneId: "scene-1",
        stage: "image",
        shotIds: expect.arrayContaining(["shot-1", "shot-100"]),
        allowPartial: false,
      });
      expect(admitMock.mock.calls[0]?.[0].shotIds).toHaveLength(100);
      expect(textContent(tree)).toContain("已加入生产队列");

      const queueButton = findSingle(tree, (element) => element.type === "button" && textContent(element) === "前往生产队列");
      (queueButton.props.onClick as () => void)();
      expect(onOpenProductionQueue).toHaveBeenCalledWith("batch-100");
      expect(startMock).not.toHaveBeenCalled();
    } finally {
      vi.doUnmock("react");
      vi.doUnmock("../../services/tauriClient");
      vi.resetModules();
    }
  });
});

const largeReadyView: ScenePreparationView = {
  ...view,
  total: 105,
  readyCount: 105,
  incompleteCount: 0,
  blockedCount: 0,
  preparedCount: 0,
  warningCount: 0,
  items: Array.from({ length: 105 }, (_, index) => summary({
    shotId: `shot-${index + 1}`,
    ordinal: index,
    name: `镜头 ${index + 1}`,
  })),
};

class HookHarness {
  private readonly values: unknown[] = [];
  private cursor = 0;

  beginRender() {
    this.cursor = 0;
  }

  useState<T>(initial: T | (() => T)): [T, (next: T | ((current: T) => T)) => void] {
    const index = this.cursor++;
    if (!(index in this.values)) {
      this.values[index] = typeof initial === "function" ? (initial as () => T)() : initial;
    }
    const setState = (next: T | ((current: T) => T)) => {
      this.values[index] = typeof next === "function"
        ? (next as (current: T) => T)(this.values[index] as T)
        : next;
    };
    return [this.values[index] as T, setState];
  }

  useMemo<T>(factory: () => T): T {
    this.cursor++;
    return factory();
  }

  useEffect(_effect: () => void | (() => void), _dependencies?: unknown[]) {
    this.cursor++;
  }
}

interface TestElement {
  type: unknown;
  props: Record<string, unknown>;
}

function isTestElement(value: unknown): value is TestElement {
  return typeof value === "object" && value !== null && "type" in value && "props" in value;
}

function materialize(node: unknown): unknown {
  if (Array.isArray(node)) return node.map(materialize);
  if (!isTestElement(node)) return node;
  if (typeof node.type === "function") return materialize(node.type(node.props));
  return { ...node, props: { ...node.props, children: materialize(node.props.children) } };
}

function findElements(node: unknown, predicate: (element: TestElement) => boolean): TestElement[] {
  if (Array.isArray(node)) return node.flatMap((child) => findElements(child, predicate));
  if (!isTestElement(node)) return [];
  const matches = predicate(node) ? [node] : [];
  return [...matches, ...findElements(node.props.children, predicate)];
}

function findSingle(node: unknown, predicate: (element: TestElement) => boolean): TestElement {
  const matches = findElements(node, predicate);
  expect(matches).toHaveLength(1);
  return matches[0]!;
}

function textContent(node: unknown): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textContent).join("");
  if (!isTestElement(node)) return "";
  return textContent(node.props.children);
}

async function flushMicrotasks() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}
