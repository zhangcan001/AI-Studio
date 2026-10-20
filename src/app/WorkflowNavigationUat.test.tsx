// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ShellHost } from "./ShellHost";
import type { AppRoute } from "./routes/types";
import { WorkflowLabPage } from "../features/workflow-lab/WorkflowLabPage";
import { useWorkflowOnboardingStore } from "../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../stores/workflowWorkspaceStore";

const tauriMocks = vi.hoisted(() => ({
  listWorkflowProductionWorkspace: vi.fn(), listWorkflowBenchmarks: vi.fn(), repairJobsStatus: vi.fn(),
}));
vi.mock("../services/tauriClient", async () => ({
  ...await vi.importActual<typeof import("../services/tauriClient")>("../services/tauriClient"), ...tauriMocks,
}));
beforeEach(() => {
  vi.clearAllMocks();
  useWorkflowOnboardingStore.getState().reset();
  useWorkflowWorkspaceStore.getState().reset();
});
afterEach(cleanup);
const returnTo: AppRoute = { kind: "create", projectId: "project-1", shotId: "shot-1", stage: "video" };
function retired(projectId: string | undefined = "project-1", destination: AppRoute | undefined = returnTo) {
  const callbacks = { onCatalogChanged: vi.fn(), onOpenStudio: vi.fn(), onUseInProject: vi.fn() };
  const navigate = vi.fn();
  render(<WorkflowLabPage projectId={projectId} catalog={[]} comfyConnected={false} navigate={navigate} returnTo={destination} initialDiagnosticsOpen {...callbacks} />);
  return { navigate, callbacks };
}

describe("Workflow authoring retirement navigation UAT", () => {
  it.each(["system-settings", "project-settings"] as const)("removes advanced workflow navigation from %s, keeping ordinary settings", kind => {
    const route: AppRoute = kind === "system-settings" ? { kind, section: "general", returnTo } : { kind, projectId: "project-1", section: "generators" };
    const navigate = vi.fn();
    render(<ShellHost route={route} navigate={navigate} back={vi.fn()} projectSelector={null}><p>settings</p></ShellHost>);
    expect(screen.queryByRole("button", { name: /高级工作流|工作流.*Benchmark/ })).toBeNull();
    expect(screen.getByRole("button", { name: "高级工具" })).toBeTruthy();
    expect(navigate).not.toHaveBeenCalled();
  });
  it("a restored legacy route does not load authoring, benchmarks, repairs, or mutate bindings", async () => {
    const { navigate, callbacks } = retired();
    await Promise.resolve();
    expect(screen.getByRole("heading", { name: "工作流导入与编辑已停用" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /添加工作流|导入|用于当前项目|Benchmark/ })).toBeNull();
    for (const callback of [...Object.values(tauriMocks), ...Object.values(callbacks), navigate]) expect(callback).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "选择项目生成器" }));
    expect(navigate).toHaveBeenCalledWith({ kind: "project-settings", projectId: "project-1", section: "generators" });
    fireEvent.click(screen.getByRole("button", { name: "返回原页面" }));
    expect(navigate).toHaveBeenLastCalledWith(returnTo);
  });
  it("a global saved route has a safe project picker instead of an import action", () => {
    const { navigate } = retired("");
    fireEvent.click(screen.getByRole("button", { name: "选择项目" }));
    expect(navigate).toHaveBeenCalledWith({ kind: "project-list" });
    expect(screen.queryByRole("button", { name: "选择项目生成器" })).toBeNull();
  });
  it.each(["system-settings", "project-settings"] as const)("does not loop back to a retired %s return locator", kind => {
    const destination: AppRoute = kind === "system-settings" ? { kind, section: "advanced-workflows" } : { kind, projectId: "project-1", section: "advanced-workflows" };
    retired("project-1", destination);
    expect(screen.queryByRole("button", { name: "返回原页面" })).toBeNull();
  });
});
