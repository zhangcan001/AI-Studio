// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ShotView } from "../../types/shot";
import type { ProjectPreparationView, ShotProductionPlanSummary } from "../../types/productionPreparation";
import { ShotBulkConfigPanel } from "./ShotBulkConfigPanel";
import { ProjectProductionPipeline } from "./ProjectProductionPipeline";

const api = vi.hoisted(() => ({ preflight: vi.fn(), admit: vi.fn(), start: vi.fn() }));
vi.mock("../../services/tauriClient", () => ({ getProjectProductionPreflight: api.preflight, admitProjectProduction: api.admit, startProductionQueue: api.start }));
const shot = (id = "shot-1", projectId = "project-1", ordinal = 0): ShotView => ({ id, projectId, ordinal, name: id, promptText: "A moving landscape", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "DRAFT", stageConfigs: [], referenceAssets: [], generationLinks: [], createdAt: "", updatedAt: "" });
const row = (shotId = "shot-1", overrides: Partial<ShotProductionPlanSummary> = {}): ShotProductionPlanSummary => ({ shotId, ordinal: 0, name: shotId, status: "READY", score: 100, warningCount: 0, incompleteCount: 0, blockerCount: 0, contextHash: "context-1", workflowVersionId: "version-t2v", recipeId: "t2v", referenceCount: 0, alreadyPrepared: false, blockers: [], ...overrides });
const plan = (items = [row()], projectId = "project-1", stage: "video" | "image" = "video"): ProjectPreparationView => ({ projectId, stage, total: items.length, readyCount: items.filter(r => r.status === "READY").length, blockedCount: 0, incompleteCount: 0, warningCount: 0, preparedCount: items.filter(r => r.alreadyPrepared).length, items });
const admitted = { projectId: "project-1", stage: "video", createdCount: 1, alreadyPreparedCount: 0, skippedIncomplete: 0, skippedBlocked: 0, createdBatchIds: ["batch-1"] };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r; }); return { promise, resolve }; }
beforeEach(() => { vi.clearAllMocks(); api.preflight.mockResolvedValue(plan()); api.admit.mockResolvedValue(admitted); });
afterEach(cleanup);

describe("Phase5A formal video storyboard preparation", () => {
  it("does not treat image selection or a successful video candidate as selection or human approval", async () => {
    const shots: ShotView[] = [{ ...shot(), selectedImageAssetId: "old-image", videoStatus: "VIDEO_REVIEW" }];
    render(<ProjectProductionPipeline projectId="project-1" shots={shots} />);
    expect(screen.getByText(/0 \/ 1 已选用视频/)).toBeTruthy();
    expect(screen.getByText(/生成成功不等于选用，选用也不等于人工审核通过/)).toBeTruthy();
    await waitFor(() => expect(api.preflight).toHaveBeenCalled());
    expect(shots[0].selectedVideoAssetId).toBeUndefined();
    expect(api.admit).not.toHaveBeenCalled();
    expect(api.start).not.toHaveBeenCalled();
  });
  it("defaults to video and prepares image-free T2V via fresh preflight and explicit admission only", async () => {
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await waitFor(() => expect(api.preflight).toHaveBeenCalledWith({ projectId: "project-1", stage: "video" }));
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" }));
    await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1));
    expect(api.preflight.mock.invocationCallOrder[1]).toBeLessThan(api.admit.mock.invocationCallOrder[0]);
    expect(api.admit).toHaveBeenCalledWith({ projectId: "project-1", stage: "video", shotIds: ["shot-1"], allowPartial: false });
    expect(api.start).not.toHaveBeenCalled();
    expect(shot().selectedImageAssetId).toBeUndefined();
  });
  it("retains image history but cannot prepare retired image generation", async () => {
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await user.click(screen.getByRole("tab", { name: /图片/ }));
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" }));
    const prepare = screen.queryByRole("button", { name: "准备图片生产" });
    expect(!prepare || prepare.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/图片生成已退休/)).toBeTruthy();
    expect(api.admit).not.toHaveBeenCalled();
  });
  it.each(["I2V 缺少首帧", "FIRST_LAST 缺少独立尾帧", "REF2VA 纯音频不可生产"])("displays formal input denial: %s", async (reason) => {
    api.preflight.mockResolvedValue(plan([row("shot-1", { status: "INCOMPLETE", blockers: [reason] })]));
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await screen.findByText(reason);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" }));
    await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.preflight).toHaveBeenCalledTimes(2));
    expect(api.admit).not.toHaveBeenCalled();
  });
  it("does not freeze client guesses; mixed FIRST_LAST/REF2VA order and exact identities remain backend-owned", async () => {
    api.preflight.mockResolvedValue(plan([row("first-last", { ordinal: 1, recipeId: "first-last", workflowVersionId: "version-fl" }), row("mixed", { ordinal: 0, recipeId: "ref2va", workflowVersionId: "version-ref", referenceCount: 3 })]));
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot("first-last", "project-1", 1), shot("mixed")]} />);
    await user.click(screen.getByRole("button", { name: "选择就绪项" }));
    await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1));
    expect(api.admit.mock.calls[0][0]).toEqual({ projectId: "project-1", stage: "video", shotIds: ["mixed", "first-last"], allowPartial: false });
  });
  it("ignores preflight from a previous project including shared shot IDs", async () => {
    const old = deferred<ProjectPreparationView>(); api.preflight.mockImplementation(({ projectId }) => projectId === "project-1" ? old.promise : Promise.resolve(plan([row("shared", { status: "BLOCKED", blockers: ["Project B blocker"] })], "project-2")));
    const { rerender } = render(<ShotBulkConfigPanel projectId="project-1" shots={[shot("shared")]} />);
    rerender(<ShotBulkConfigPanel projectId="project-2" shots={[shot("shared", "project-2")]} />);
    await screen.findByText("Project B blocker"); await act(async () => old.resolve(plan([row("shared", { blockers: ["OLD PROJECT RESULT"] })])));
    expect(screen.queryByText("OLD PROJECT RESULT")).toBeNull();
  });
  it("ignores late admission, callbacks and queue navigation after project switch", async () => {
    const old = deferred<typeof admitted>(); api.admit.mockReturnValue(old.promise); const notice = vi.fn(); const refresh = vi.fn();
    api.preflight.mockImplementation(({ projectId }) => Promise.resolve(plan([row()], projectId)));
    const user = userEvent.setup(); const { rerender } = render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} onNotice={notice} onRefresh={refresh} />);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" })); await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1)); notice.mockClear();
    rerender(<ShotBulkConfigPanel projectId="project-2" shots={[shot("shot-1", "project-2")]} onNotice={notice} onRefresh={refresh} />);
    await act(async () => old.resolve(admitted));
    expect(notice).not.toHaveBeenCalled(); expect(refresh).not.toHaveBeenCalled(); expect(screen.queryByText(/已准备完成/)).toBeNull();
  });
  it("fresh readiness denial blocks stale selection without creating a batch", async () => {
    api.preflight.mockResolvedValueOnce(plan()).mockResolvedValue(plan([row("shot-1", { status: "BLOCKED", blockers: ["上下文已变更"] })]));
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" })); await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await screen.findByRole("alert"); expect(api.admit).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
  });
  it("retains the admission receipt and queue link without reloading Shot drafts or starting execution", async () => {
    const onRefresh = vi.fn(), openQueue = vi.fn();
    api.preflight.mockResolvedValueOnce(plan()).mockResolvedValueOnce(plan())
      .mockResolvedValue(plan([row("shot-1", { alreadyPrepared: true, matchingPreparedBatchIds: ["batch-1"] })]));
    const user = userEvent.setup();
    render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} onRefresh={onRefresh} onOpenProductionQueue={openQueue} />);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" }));
    await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await screen.findByText(/已准备完成/);
    await user.click(screen.getByRole("button", { name: "打开生产队列" }));
    expect(openQueue).toHaveBeenCalledExactlyOnceWith("batch-1");
    expect(onRefresh).not.toHaveBeenCalled();
    expect(api.start).not.toHaveBeenCalled();
  });
  it("rejects a superseded preflight after a Shot edit rather than reporting fictitious admission success", async () => {
    const pending = deferred<ProjectPreparationView>();
    api.preflight.mockResolvedValueOnce(plan()).mockReturnValueOnce(pending.promise).mockResolvedValue(plan());
    const notice = vi.fn(), user = userEvent.setup();
    const { rerender } = render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} onNotice={notice} />);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" }));
    await user.click(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.preflight).toHaveBeenCalledTimes(2));
    rerender(<ShotBulkConfigPanel projectId="project-1" shots={[{ ...shot(), promptText: "Changed draft" }]} onNotice={notice} />);
    await waitFor(() => expect(api.preflight).toHaveBeenCalledTimes(3));
    await act(async () => pending.resolve(plan()));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("预检已失效"));
    expect(notice.mock.calls.some(([message]) => String(message).includes("准备操作已完成"))).toBe(false);
    expect(api.admit).not.toHaveBeenCalled();
    expect(api.start).not.toHaveBeenCalled();
  });
  it("serializes duplicate clicks and refreshes matching/stale preparation from the server", async () => {
    const pending = deferred<typeof admitted>(); api.admit.mockReturnValue(pending.promise);
    api.preflight.mockResolvedValueOnce(plan()).mockResolvedValueOnce(plan()).mockResolvedValue(plan([row("shot-1", { alreadyPrepared: true, matchingPreparedBatchIds: ["batch-1"], stalePreparedBatchIds: ["old-batch"] })]));
    const user = userEvent.setup(); render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await user.click(screen.getByRole("checkbox", { name: "选择 shot-1" })); await user.dblClick(screen.getByRole("button", { name: "准备视频生产" }));
    await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1)); await act(async () => pending.resolve(admitted));
    await screen.findByText(/陈旧/); expect(api.start).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "选择就绪项" }));
    expect((screen.getByRole("checkbox", { name: "选择 shot-1" }) as HTMLInputElement).checked).toBe(false);
  });
  it("rejects project-owned rows that do not belong to this panel", async () => {
    api.preflight.mockResolvedValue(plan([row("foreign")]));
    render(<ShotBulkConfigPanel projectId="project-1" shots={[shot()]} />);
    await screen.findByRole("alert"); expect(api.admit).not.toHaveBeenCalled();
  });
});
