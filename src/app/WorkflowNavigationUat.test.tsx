// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { ShellHost } from "./ShellHost";
import type { AppRoute } from "./routes/types";
import { WorkflowLabPage } from "../features/workflow-lab/WorkflowLabPage";
import { useWorkflowOnboardingStore } from "../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../stores/workflowWorkspaceStore";
import type { WorkflowProductionWorkspaceResponse } from "../types/workflowOnboarding";

const tauriMocks = vi.hoisted(() => ({
  listWorkflowProductionWorkspace: vi.fn(),
  listWorkflowBenchmarks: vi.fn(async () => []),
  repairJobsStatus: vi.fn(async () => []),
}));

vi.mock("../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../services/tauriClient")>("../services/tauriClient");
  return { ...actual, ...tauriMocks };
});

const EMPTY_WORKSPACE = {
  items: [],
  staging: [],
} satisfies WorkflowProductionWorkspaceResponse;

beforeEach(() => {
  vi.clearAllMocks();
  tauriMocks.listWorkflowProductionWorkspace.mockResolvedValue(EMPTY_WORKSPACE);
  useWorkflowOnboardingStore.getState().reset();
  useWorkflowWorkspaceStore.getState().reset();
});

afterEach(() => cleanup());

describe("DEV-079 工作流导航闭环 UAT", () => {
  it("从 V3 高级入口进入真实 WorkflowLabPage", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();

    function Harness() {
      const [route,setRoute]=useState<AppRoute>({kind:"system-settings",section:"general",returnTo:{kind:"project",projectId:"project-1",page:"overview"}});
      const navigate=(next:AppRoute)=>{onNavigate(next);setRoute(next);};
      return <ShellHost route={route} navigate={navigate} back={vi.fn()} projectSelector={null}>
        {route.kind === "system-settings" && route.section === "advanced-workflows" && <WorkflowLabPage navigate={navigate} projectId="project-1" catalog={[]} comfyConnected={false} onCatalogChanged={async()=>undefined} onOpenStudio={async()=>undefined} onUseInProject={async()=>undefined}/>}
      </ShellHost>;
    }
    render(<Harness/>);
    await user.click(screen.getByRole("button",{name:"高级工作流"}));
    await waitFor(()=>expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
    expect(onNavigate).toHaveBeenCalledWith(expect.objectContaining({kind:"system-settings",section:"advanced-workflows"}));
    expect(screen.getByRole("heading", { name: "工作流管理" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "+ 添加工作流" })).toBeTruthy();
  });
});
