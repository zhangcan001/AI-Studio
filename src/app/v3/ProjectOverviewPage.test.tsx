// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ProjectOverviewPage, overviewAction } from "./ProjectOverviewPage";
import { runtimeReadinessPresentation } from "./projectOverviewReadiness";
import type { ProjectOverview } from "../../product/types";
const mocks = vi.hoisted(() => ({ getOverview: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { project: mocks } }));
beforeEach(() => { vi.resetAllMocks(); });
afterEach(cleanup);
const unknown: ProjectOverview["runtimeReadiness"] = { connection: null, status: null, workflowReady: 0, workflowTotal: 0, runtimeBusy: false, activeTaskCount: 0, productionBusy: false };
const ready: ProjectOverview["runtimeReadiness"] = { ...unknown, connection: "CONNECTED", status: "READY", workflowReady: 5, workflowTotal: 5 };
function fixture(facts = unknown, id = "A", kind = "NO_SHOTS"): ProjectOverview {
  return { project: { id, name: `项目 ${id}`, description: null, createdAt: "", updatedAt: "" },
    progress: { total: 0, completed: 0, failed: 0 }, blockingState: "HEALTHY",
    nextAction: { kind, priority: 1, reasonCode: kind, reason: "private reason", shotId: null, taskId: null, batchId: null, assetId: null },
    blockingIssues: [], activeRuns: { runningBatches: 0, pausedBatches: 0, activeTasks: 0 }, recentResults: { total: 0, images: 0, videos: 0 }, runtimeReadiness: facts };
}
async function page(facts = unknown, kind = "NO_SHOTS") {
  mocks.getOverview.mockResolvedValue(fixture(facts, "A", kind));
  const navigate = vi.fn(); render(<ProjectOverviewPage projectId="A" navigate={navigate} />);
  const region = within(await screen.findByRole("region", { name: "运行准备" }));
  return { navigate, region };
}
it.each([null, "CONNECTED", "OFFLINE"])("never presents null preflight as zero workflows or idle with connection %s", async connection => {
  const { region } = await page({ ...unknown, connection });
  expect(region.getByText(`连接：${connection === null ? "未检查" : connection === "CONNECTED" ? "已连接" : "离线"}`)).toBeTruthy();
  for (const label of ["运行预检：尚未预检", "生产工作流：未检查", "运行资源：未检查"]) expect(region.getByText(label)).toBeTruthy();
  expect(region.queryByText(/0 \/ 0|空闲|就绪|已通过/)).toBeNull();
  expect(screen.getByText("暂无项目阻断")).toBeTruthy();
});
it.each(["INCOMPATIBLE", "FUTURE_STATE", "__proto__"])("safely presents connection %s without guessing online", async connection => {
  const { region } = await page({ ...unknown, connection });
  expect(region.getByText(`连接：${connection === "INCOMPATIBLE" ? "版本不兼容" : "未知"}`)).toBeTruthy();
});
it("shows connected and ready cached facts without another primary CTA", async () => {
  const { region } = await page(ready);
  for (const label of ["连接：已连接", "运行预检：已通过", "生产工作流：5 / 5 可用", "运行资源：空闲"]) expect(region.getByText(label)).toBeTruthy();
  expect(region.queryByRole("button")).toBeNull(); expect(document.querySelectorAll(".primary-button")).toHaveLength(1);
});
it.each(["WARNING", "BLOCKED"] as const)("presents %s and exact secondary settings return", async status => {
  const { region, navigate } = await page({ ...ready, status, workflowReady: 4 });
  expect(region.getByText(`运行预检：${status === "WARNING" ? "有警告" : "已阻断"}`)).toBeTruthy();
  expect(region.getByText("生产工作流：4 / 5 可用")).toBeTruthy();
  fireEvent.click(region.getByRole("button", { name: "检查运行环境" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "system-settings", section: "general", returnTo: { kind: "project", projectId: "A", page: "overview" } });
});
it.each([{ runtimeBusy: true }, { productionBusy: true }, { activeTaskCount: 2 }])("explains busy resources without recomputing nextAction %j", async patch => {
  const { region, navigate } = await page({ ...ready, ...patch });
  expect(region.getByText(patch.activeTaskCount ? "运行资源：忙碌 · 活动任务 2" : "运行资源：忙碌")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "创建第一个镜头" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "A", stage: "image" });
});
it("keeps an empty offline project's Create CTA and settings action independent", async () => {
  const { region, navigate } = await page({ ...unknown, connection: "OFFLINE" });
  fireEvent.click(screen.getByRole("button", { name: "创建第一个镜头" }));
  expect(navigate).toHaveBeenLastCalledWith({ kind: "create", projectId: "A", stage: "image" });
  fireEvent.click(region.getByRole("button", { name: "检查运行环境" }));
  expect(navigate).toHaveBeenLastCalledWith({ kind: "system-settings", section: "general", returnTo: { kind: "project", projectId: "A", page: "overview" } });
  expect(mocks.getOverview).toHaveBeenCalledTimes(1);
});
it("keeps COMFY_BLOCKED primary and secondary destinations identical", async () => {
  const { region, navigate } = await page({ ...ready, status: "BLOCKED" }, "COMFY_BLOCKED");
  fireEvent.click(document.querySelector<HTMLButtonElement>(".primary-button")!);
  const expected = overviewAction("A", fixture(ready, "A", "COMFY_BLOCKED").nextAction).route;
  expect(navigate).toHaveBeenLastCalledWith(expected);
  fireEvent.click(region.getByRole("button", { name: "检查运行环境" })); expect(navigate).toHaveBeenLastCalledWith(expected);
});
it("ignores preflight placeholders even when busy/count placeholders are nonzero", () => {
  expect(runtimeReadinessPresentation({ ...unknown, runtimeBusy: true, activeTaskCount: 2, workflowTotal: 5 })).toMatchObject({ workflowsLabel: "未检查", resourcesLabel: "未检查" });
});
it.each(["resolve", "reject"])("does not leak late A readiness or error into B (%s)", async outcome => {
  let resolveA!: (v: ProjectOverview) => void; let rejectA!: (e: unknown) => void;
  mocks.getOverview.mockImplementation((id: string) => id === "A" ? new Promise<ProjectOverview>((resolve, reject) => { resolveA = resolve; rejectA = reject; }) : Promise.resolve(fixture(ready, "B")));
  const { rerender } = render(<ProjectOverviewPage projectId="A" navigate={vi.fn()} />);
  rerender(<ProjectOverviewPage projectId="B" navigate={vi.fn()} />); await screen.findByRole("heading", { name: "项目 B" });
  await act(async () => { if (outcome === "resolve") resolveA(fixture({ ...unknown, connection: "OFFLINE" })); else rejectA(new Error("A private error")); });
  expect(screen.queryByText("连接：离线")).toBeNull(); expect(screen.queryByRole("alert")).toBeNull(); expect(screen.getByText("连接：已连接")).toBeTruthy();
});
