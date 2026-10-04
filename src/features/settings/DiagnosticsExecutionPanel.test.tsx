// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DiagnosticExecutionHealth, DiagnosticRecentFailure, DiagnosticTaskTimeline } from "../../types/diagnostics";
import { DiagnosticsExecutionPanel } from "./DiagnosticsExecutionPanel";
import { SettingsWorkspace } from "./SettingsWorkspace";

const mocks = vi.hoisted(() => ({ health: vi.fn(), failures: vi.fn(), timeline: vi.fn(), summary: vi.fn() }));
vi.mock("../../services/diagnosticsClient", () => ({ diagnosticsClient: {
  executionHealth: mocks.health, recentFailures: mocks.failures, taskTimeline: mocks.timeline,
  summary: mocks.summary,
} }));
vi.mock("../../services/tauriClient", () => ({
  getComfySettings: vi.fn().mockResolvedValue({ endpoint: "http://127.0.0.1:1" }),
  listComfyEnvironmentProfiles: vi.fn().mockResolvedValue([]),
}));
beforeEach(() => vi.clearAllMocks());
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const health: DiagnosticExecutionHealth = {
  projectId: "project-a", windowLimit: 50, sampleCount: 1, active: 0, succeeded: 0, failed: 1, cancelled: 0,
  telemetryComplete: 0, telemetryPartial: 0, telemetryUnavailable: 1, telemetryInvalid: 0,
  prepare: { medianMs: null, sampleCount: 0 }, submit: { medianMs: null, sampleCount: 0 },
  queueWait: { medianMs: null, sampleCount: 0 }, execution: { medianMs: null, sampleCount: 0 },
  collection: { medianMs: null, sampleCount: 0 }, total: { medianMs: null, sampleCount: 0 },
};
const failure: DiagnosticRecentFailure = {
  projectId: "project-a", exampleTaskId: "tsk_private_a", code: "COMFY_OFFLINE", status: "FAILED",
  count: 1, affectedTaskCount: 1, latestAt: null,
};
const timeline: DiagnosticTaskTimeline = {
  projectId: "project-a", taskId: "tsk_private_a", status: "FAILED", completeness: "LEGACY_UNAVAILABLE",
  durations: { prepareMs: null, submitMs: null, queueWaitMs: null, executionMs: null, collectionMs: null, totalMs: null },
  createdAt: "2026-10-03T08:00:00Z", queuedAt: null, startedAt: null, finishedAt: "2026-10-03T09:00:00Z",
  prepareStartedAt: null, preparedAt: null, submittedAt: null, executionStartedAt: null,
  executionFinishedAt: null, collectionFinishedAt: null, generationExecutionId: null, runtimeProfile: null, concurrencyClass: null,
};

describe("Phase13 diagnostics UI/deep links/privacy", () => {
  it("clears formerly healthy summary when a refresh fails instead of showing stale healthy facts", async () => {
    mocks.summary.mockResolvedValueOnce({ appVersion: "TEST_VERSION", platform: "test", architecture: "test", runMode: "test", databaseHealthy: true,
      comfyStatus: "OFFLINE", workflowPackages: 5, validWorkflowPackages: 5, activeTaskCount: 0, productionBusy: false, loggingAvailable: true, logRetentionDays: 7,
    }).mockRejectedValueOnce(new Error("isolated summary read failed"));
    render(<SettingsWorkspace connectionLoading={false} capabilityLoading={false} onReconnect={vi.fn()} onRefreshCapabilities={vi.fn()} showWorkflowRepairStatus={false} />);
    expect(await screen.findByText("TEST_VERSION")).toBeTruthy();
    expect(screen.getByText("正常")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "刷新诊断" }));
    expect((await screen.findByRole("alert")).textContent).toBeTruthy();
    expect(screen.queryByText("TEST_VERSION")).toBeNull();
    expect(screen.queryByText("正常")).toBeNull();
    expect(screen.queryByText("空闲")).toBeNull();
    expect(screen.getAllByText("未知").length).toBeGreaterThan(0);
  });

  it("ignores stale project reads, has one refresh owner, and starts no polling", async () => {
    let resolveOld!: (value: DiagnosticExecutionHealth) => void;
    const old = new Promise<DiagnosticExecutionHealth>(resolve => { resolveOld = resolve; });
    mocks.health.mockReturnValueOnce(old).mockResolvedValue({ ...health, projectId: "project-b", sampleCount: 7 });
    mocks.failures.mockResolvedValue([]);
    const interval = vi.spyOn(globalThis, "setInterval");
    const { rerender, unmount } = render(<DiagnosticsExecutionPanel projectId="project-a" />);
    expect(mocks.health).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "刷新执行诊断" }));
    expect(mocks.health).toHaveBeenCalledTimes(1);
    await act(async () => { rerender(<DiagnosticsExecutionPanel projectId="project-b" />); });
    expect(screen.getByText("当前项目最近 50 个任务 · 实际 7 个记录，不代表完整历史。")).toBeTruthy();
    await act(async () => { resolveOld(health); await old; });
    expect(screen.queryByText("当前项目最近 50 个任务 · 实际 1 个记录，不代表完整历史。")).toBeNull();
    expect(mocks.health).toHaveBeenCalledTimes(2);
    expect(interval).not.toHaveBeenCalled();
    unmount();
  });

  it("rejects a wrong-project response rather than rendering healthy facts", async () => {
    mocks.health.mockResolvedValue({ ...health, projectId: "wrong-project" });
    mocks.failures.mockResolvedValue([]);
    render(<DiagnosticsExecutionPanel projectId="project-a" />);
    expect((await screen.findByRole("alert")).textContent).toBeTruthy();
    expect(screen.queryByText("阶段耗时中位数")).toBeNull();
  });

  it("shows bounded project health, legacy null timeline and delegates exact task links", async () => {
    mocks.health.mockResolvedValue(health);
    mocks.failures.mockResolvedValue([failure]);
    mocks.timeline.mockResolvedValue(timeline);
    const onOpenRun = vi.fn(), onOpenAudit = vi.fn();
    const { container } = render(<DiagnosticsExecutionPanel projectId="project-a" initialTaskId="tsk_private_a" onOpenRun={onOpenRun} onOpenAudit={onOpenAudit} />);

    expect(await screen.findByText("当前项目最近 50 个任务 · 实际 1 个记录，不代表完整历史。")).toBeTruthy();
    expect(await screen.findByText("旧任务未采集完整执行遥测")).toBeTruthy();
    expect(screen.getAllByText("未知").length).toBeGreaterThan(0);
    expect(screen.queryByText("0 ms")).toBeNull();
    expect(container.textContent).toContain("COMFY_OFFLINE");
    expect(container.textContent).toContain("时间未记录");
    expect(container.textContent).not.toContain("PRIVATE_PROMPT");

    fireEvent.click(screen.getAllByRole("button", { name: "查看运行" })[0]);
    fireEvent.click(screen.getAllByRole("button", { name: "查看生产链路" })[0]);
    expect(onOpenRun).toHaveBeenCalledWith("project-a", "tsk_private_a");
    expect(onOpenAudit).toHaveBeenCalledWith("project-a", "tsk_private_a");
    fireEvent.click(screen.getByRole("button", { name: "刷新执行诊断" }));
    await waitFor(() => expect(mocks.health).toHaveBeenCalledTimes(2));
    expect(mocks.failures).toHaveBeenCalledTimes(2);
  });
});
