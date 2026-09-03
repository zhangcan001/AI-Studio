// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StudioShell } from "./StudioShell";
import { WorkflowWorkspace } from "../features/workflows/WorkflowWorkspace";
import { useWorkflowOnboardingStore } from "../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../stores/workflowWorkspaceStore";
import type { WorkflowProductionWorkspaceResponse } from "../types/workflowOnboarding";

const tauriMocks = vi.hoisted(() => ({
  listWorkflowProductionWorkspace: vi.fn(),
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
  it("从真实 StudioShell 的工作流 rail 进入真实 WorkflowWorkspace", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();

    render(
      <StudioShell
        workspace="workflows"
        currentSection="workflows"
        onNavigate={onNavigate}
      >
        <WorkflowWorkspace
          projectId="project-1"
          catalog={[]}
          comfyConnected={false}
          onCatalogChanged={async () => undefined}
          onOpenStudio={async () => undefined}
          onUseInProject={async () => undefined}
        />
      </StudioShell>,
    );

    await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: /^工作流：/ }));

    expect(onNavigate).toHaveBeenCalledWith(
      "workflows",
      expect.objectContaining({ id: "workflows" }),
    );
    expect(screen.getByRole("heading", { name: "工作流管理" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "+ 添加工作流" })).toBeTruthy();
  });
});
