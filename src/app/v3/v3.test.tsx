// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShellV3 } from "./AppShellV3";
import { overviewAction, ProjectOverviewPage } from "./ProjectOverviewPage";
import { useAppRoute } from "../routes/useAppRoute";
import { useDraftConfirmation } from "../routes/useDraftConfirmation";
import { useStudioStore } from "../../stores/studioStore";
import type { ProjectOverview } from "../../product/types";
const mocks = vi.hoisted(() => ({ getOverview: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { project: mocks } }));
beforeEach(() => { vi.clearAllMocks(); localStorage.clear(); });
afterEach(() => { cleanup(); useStudioStore.getState().resetDraft(); });
const overview: ProjectOverview = {
  project: { id: "A", name: "项目 A", description: null, createdAt: "", updatedAt: "" },
  progress: { total: 0, completed: 0, failed: 0 }, blockingState: "HEALTHY",
  nextAction: { kind: "NO_SHOTS", priority: 1, reasonCode: "NO_SHOTS", reason: "internal", shotId: null, taskId: null, batchId: null, assetId: null },
  blockingIssues: [], activeRuns: { runningBatches: 0, pausedBatches: 0, activeTasks: 0 },
  recentResults: { total: 0, images: 0, videos: 0 },
  runtimeReadiness: { status: "READY", connection: null, workflowReady: 1, workflowTotal: 1, runtimeBusy: false, activeTaskCount: 0, productionBusy: false },
};
describe("project-first shell and facade overview", () => {
  it("waits for an explicit draft decision and defaults to continuing editing", async () => {
    const originalShow = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, "showModal");
    const originalClose = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, "close");
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", { configurable: true, value() { this.open = true; } });
    Object.defineProperty(HTMLDialogElement.prototype, "close", { configurable: true, value() { this.open = false; } });
    let decision: Promise<boolean> | undefined;
    function Fixture() { const { confirm, dialog } = useDraftConfirmation(); return <><button onClick={() => { decision = confirm("未保存的草稿"); }}>离开</button>{dialog}</>; }
    try {
      render(<Fixture />);
      fireEvent.click(screen.getByRole("button", { name: "离开" }));
      expect(screen.getByRole("dialog").textContent).toContain("未保存的草稿");
      fireEvent.click(screen.getByRole("button", { name: "继续编辑" }));
      await expect(decision).resolves.toBe(false);
      fireEvent.click(screen.getByRole("button", { name: "离开" }));
      fireEvent.click(screen.getByRole("button", { name: "放弃修改并继续" }));
      await expect(decision).resolves.toBe(true);
    } finally {
      cleanup();
      if (originalShow) Object.defineProperty(HTMLDialogElement.prototype, "showModal", originalShow); else Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
      if (originalClose) Object.defineProperty(HTMLDialogElement.prototype, "close", originalClose); else Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
    }
  });
  it("shows five primary pages, secondary settings and canonical navigation", () => {
    const navigate = vi.fn(); const back = vi.fn();
    render(<AppShellV3 route={{ kind: "project", projectId: "A", page: "overview" }} projectName="项目 A" projectSelector={<select aria-label="当前项目"><option>A</option></select>} navigate={navigate} back={back}><p>content</p></AppShellV3>);
    expect(screen.getByRole("navigation", { name: "项目导航" }).querySelectorAll("button")).toHaveLength(5);
    expect(screen.getByRole("button", { name: "概览" }).getAttribute("aria-current")).toBe("page");
    expect(screen.queryByRole("button", { name: "工作流" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "运行" }));
    expect(navigate).toHaveBeenLastCalledWith({ kind: "runs", projectId: "A" });
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(navigate).toHaveBeenLastCalledWith({ kind: "create", projectId: "A", stage: "video" });
    fireEvent.click(screen.getByRole("button", { name: "返回" })); expect(back).toHaveBeenCalled();
  });
  it("fixes empty project primary CTA and hides technical IDs", async () => {
    mocks.getOverview.mockResolvedValue(overview); const navigate = vi.fn();
    render(<ProjectOverviewPage projectId="A" navigate={navigate} />);
    fireEvent.click(await screen.findByRole("button", { name: "创建第一个镜头" }));
    expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "A", stage: "video" });
    expect(document.querySelectorAll(".primary-button")).toHaveLength(1);
    expect(document.body.textContent).not.toContain("internal");
    expect(mocks.getOverview).toHaveBeenCalledWith("A");
  });
  it("translates backend-selected actions without recalculating priorities", () => {
    const expected = { ACTIVE_PRODUCTION: "runs", REVIEW_REQUIRED: "runs", IMAGE_REVIEW: "create", VIDEO_REVIEW: "create", READY: "create", COMFY_BLOCKED: "system-settings", COMPLETE: "library" } as const;
    for (const [kind, routeKind] of Object.entries(expected)) expect(overviewAction("A", { ...overview.nextAction, kind, shotId: "shot", taskId: "task", assetId: "asset" }).route.kind).toBe(routeKind);
  });
  it("does not let a late A response leak into B", async () => {
    let resolveA!: (value: ProjectOverview) => void;
    mocks.getOverview.mockImplementation((id: string) => id === "A" ? new Promise<ProjectOverview>((resolve) => { resolveA = resolve; }) : Promise.resolve({ ...overview, project: { ...overview.project, id: "B", name: "项目 B" } }));
    const { rerender } = render(<ProjectOverviewPage projectId="A" navigate={vi.fn()} />);
    rerender(<ProjectOverviewPage projectId="B" navigate={vi.fn()} />);
    await screen.findByRole("heading", { name: "项目 B" });
    await act(async () => { resolveA(overview); });
    expect(screen.queryByRole("heading", { name: "项目 A" })).toBeNull();
  });
  it("preserves existing StudioStore draft through same-project navigation", async () => {
    useStudioStore.getState().setValue("prompt", { type: "string", value: "unsaved draft" });
    const { result } = renderHook(() => useAppRoute());
    act(() => result.current.restore({ kind: "create", projectId: "A", stage: "video", surface: "batch" }));
    act(() => result.current.navigate({ kind: "project", projectId: "A", page: "overview" }));
    act(() => result.current.navigate({ kind: "create", projectId: "A", stage: "video", surface: "batch" }));
    expect(useStudioStore.getState().draftDirty).toBe(true);
    expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "unsaved draft" });
    await waitFor(() => expect(localStorage.getItem("aistudio.appRoute.v2")).toContain('"create"'));
  });
});
