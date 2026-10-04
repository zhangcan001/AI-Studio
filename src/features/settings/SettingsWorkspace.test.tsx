// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SettingsWorkspace } from "./SettingsWorkspace";
import type { ComfyPreflightReport } from "../../types/settings";
const api = vi.hoisted(() => ({ getComfySettings: vi.fn(), listComfyEnvironmentProfiles: vi.fn(), testComfyConnection: vi.fn(), saveComfyEndpoint: vi.fn(), applyComfyEnvironmentProfile: vi.fn(), getComfyPreflight: vi.fn(), summary: vi.fn() }));
vi.mock("../../services/tauriClient", async () => ({ ...await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient"), ...api }));
vi.mock("../../services/diagnosticsClient", () => ({ diagnosticsClient: { summary: api.summary } }));
vi.mock("./DiagnosticsExecutionPanel", () => ({ DiagnosticsExecutionPanel: () => <section aria-label="执行诊断">任务诊断与执行遥测</section> }));
vi.mock("./RepairJobsStatusSection", () => ({ RepairJobsStatusSection: () => <section aria-label="修复任务">修复任务状态</section> }));
const applied = { schemaVersion: 1, endpoint: "http://127.0.0.1:8188" };
const profile = { id: "owned-profile", name: "测试环境", endpoint: "http://127.0.0.1:9999", createdAt: "", updatedAt: "" };
const report: ComfyPreflightReport = { endpoint: applied.endpoint, status: "READY", checkedAt: "", connection: "CONNECTED", runtimeBusy: false, activeTaskCount: 0, productionBusy: false, workflowSummary: { workflowReady: 5, workflowTotal: 5, workflowBlocked: 0 }, issues: [] };
beforeEach(() => {
  vi.resetAllMocks(); api.getComfySettings.mockResolvedValue(applied); api.listComfyEnvironmentProfiles.mockResolvedValue([profile]);
  api.summary.mockResolvedValue({ appVersion: "2.0.0-personal", platform: "windows", architecture: "x64", runMode: "test", databaseHealthy: true, comfyStatus: "CONNECTED", workflowPackages: 5, validWorkflowPackages: 5, invalidWorkflowPackages: 0, activeTaskCount: 0, productionBusy: false, loggingAvailable: true, logRetentionDays: 7 });
  api.testComfyConnection.mockResolvedValue({ connected: true, endpoint: profile.endpoint, version: "fixture", gpu: [], nodeCount: 5 });
  api.saveComfyEndpoint.mockImplementation(async endpoint => ({ ...applied, endpoint }));
  api.applyComfyEnvironmentProfile.mockResolvedValue({ ...applied, endpoint: profile.endpoint }); api.getComfyPreflight.mockResolvedValue(report);
});
afterEach(cleanup);
async function page(props: Partial<Parameters<typeof SettingsWorkspace>[0]> = {}) {
  const callbacks = { onEndpointApplied: vi.fn(), onOpenProjectGenerators: vi.fn(), onOpenToolHub: vi.fn() };
  render(<SettingsWorkspace connectionLoading={false} capabilityLoading={false} onReconnect={vi.fn()} onRefreshCapabilities={vi.fn()} {...callbacks} {...props} />);
  await waitFor(() => expect((screen.getByLabelText("ComfyUI 地址") as HTMLInputElement).value).toBe(applied.endpoint));
  return callbacks;
}
it("orders runtime setup/configuration/preflight ahead of summary and deep diagnostics without deleting them", async () => {
  await page(); const ids = [...document.querySelectorAll("h3,h4")].map(n => n.textContent);
  expect(ids.indexOf("运行环境准备")).toBeLessThan(ids.indexOf("ComfyUI 运行环境"));
  expect(screen.getByRole("link", { name: "配置并应用 ComfyUI 地址" }).getAttribute("href")).toBe("#comfy-endpoint");
  expect(screen.getByRole("link", { name: "执行运行预检" }).getAttribute("href")).toBe("#settings-preflight-title");
  expect(ids.indexOf("ComfyUI 运行环境")).toBeLessThan(ids.indexOf("运行预检"));
  expect(ids.indexOf("运行预检")).toBeLessThan(ids.indexOf("应用信息"));
  const preflight = screen.getByRole("region", { name: "运行预检" });
  expect(preflight.compareDocumentPosition(screen.getByRole("region", { name: "执行诊断" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.getByRole("region", { name: "修复任务" })).toBeTruthy(); expect(screen.getByRole("button", { name: "导出诊断包" })).toBeTruthy();
});
it("tests only the draft endpoint, explains unapplied state and never saves/applies it", async () => {
  const cb = await page(); fireEvent.change(screen.getByLabelText("ComfyUI 地址"), { target: { value: profile.endpoint } });
  expect(screen.getByText("尚未应用")).toBeTruthy(); expect(screen.getByText(/测试连接仅测试.*不会保存或应用/)).toBeTruthy(); expect(screen.getByText(/当前输入只用于测试/)).toBeTruthy();
  fireEvent.click(within(screen.getByLabelText("ComfyUI 地址").closest(".settings-endpoint-form") as HTMLElement).getByRole("button", { name: "测试连接" }));
  await screen.findByText(/已连接：fixture/);
  expect(api.testComfyConnection).toHaveBeenCalledWith(profile.endpoint);
  expect(api.saveComfyEndpoint).not.toHaveBeenCalled(); expect(api.applyComfyEnvironmentProfile).not.toHaveBeenCalled(); expect(cb.onEndpointApplied).not.toHaveBeenCalled();
});
it("explicit save/apply uses existing service, updates applied endpoint and clears test result", async () => {
  const cb = await page(); fireEvent.change(screen.getByLabelText("ComfyUI 地址"), { target: { value: profile.endpoint } });
  fireEvent.click(within(screen.getByLabelText("ComfyUI 地址").closest(".settings-endpoint-form") as HTMLElement).getByRole("button", { name: "测试连接" })); await screen.findByText(/已连接：fixture/);
  fireEvent.click(screen.getByRole("button", { name: "保存并应用" })); await screen.findByText("ComfyUI 地址已保存并应用。");
  expect(api.saveComfyEndpoint).toHaveBeenCalledWith(profile.endpoint); expect(cb.onEndpointApplied).toHaveBeenCalledTimes(1);
  expect(screen.queryByText("尚未应用")).toBeNull(); expect(screen.queryByText(/已连接：fixture/)).toBeNull();
  expect((screen.getByLabelText("ComfyUI 地址") as HTMLInputElement).value).toBe(profile.endpoint);
});
it("profile test rereads connection without switching current environment", async () => {
  const cb = await page(); const card = within(await screen.findByRole("article", { name: "ComfyUI 环境 测试环境" }));
  fireEvent.click(card.getByRole("button", { name: "测试" })); await card.findByText(/已连接 · ComfyUI fixture/);
  expect(api.testComfyConnection).toHaveBeenCalledWith(profile.endpoint); expect(api.applyComfyEnvironmentProfile).not.toHaveBeenCalled(); expect(api.saveComfyEndpoint).not.toHaveBeenCalled(); expect(cb.onEndpointApplied).not.toHaveBeenCalled();
  expect((screen.getByLabelText("ComfyUI 地址") as HTMLInputElement).value).toBe(applied.endpoint);
});
it("profile apply invokes real apply contract and updates current label before existing preflight", async () => {
  const cb = await page(); const card = within(await screen.findByRole("article", { name: "ComfyUI 环境 测试环境" }));
  fireEvent.click(card.getByRole("button", { name: "应用" })); await screen.findByText("已切换到 测试环境。");
  expect(api.applyComfyEnvironmentProfile).toHaveBeenCalledWith(profile.id); expect(cb.onEndpointApplied).toHaveBeenCalledTimes(1);
  expect(card.getByText("当前环境")).toBeTruthy(); expect(api.getComfyPreflight).toHaveBeenCalledTimes(1);
});
it("explains readonly preflight without guaranteeing real generation", async () => {
  await page(); expect(screen.getByText(/只读检查当前已应用环境.*不会切换环境、启动生成或修改工作流与模型/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "立即预检" })); await screen.findByText(/当前已知依赖可用/);
  expect(api.getComfyPreflight).toHaveBeenCalledWith(); expect(api.saveComfyEndpoint).not.toHaveBeenCalled();
  expect(screen.getByText(/预检不保证真实生成成功/)).toBeTruthy();
});
it("offers exact current-project generator callback, separate from global runtime", async () => {
  const cb = await page({ projectId: "project-owned" }); fireEvent.click(screen.getByRole("button", { name: "检查当前项目生成器" }));
  expect(cb.onOpenProjectGenerators).toHaveBeenCalledWith("project-owned"); expect(api.saveComfyEndpoint).not.toHaveBeenCalled();
  expect(screen.getByText(/保存地址不会自动绑定生成器/)).toBeTruthy();
});
it("shows explanation rather than a dead generator button without a project", async () => {
  await page(); expect(screen.queryByRole("button", { name: "检查当前项目生成器" })).toBeNull(); expect(screen.getByText(/选择项目后/)).toBeTruthy();
});
it("opens secondary Tool Hub through callback only", async () => {
  const cb = await page(); fireEvent.click(screen.getByRole("button", { name: "查看本地工具登记（高级）" }));
  expect(cb.onOpenToolHub).toHaveBeenCalledTimes(1); expect(api.testComfyConnection).not.toHaveBeenCalled(); expect(api.saveComfyEndpoint).not.toHaveBeenCalled();
});
