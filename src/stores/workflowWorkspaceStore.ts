import { create } from "zustand";
import type { WorkflowProductionWorkspaceResponse } from "../types/workflowOnboarding";

interface WorkflowWorkspaceState {
  workspace?: WorkflowProductionWorkspaceResponse;
  loadedAt?: number;
  setWorkspace: (workspace: WorkflowProductionWorkspaceResponse) => void;
  invalidate: () => void;
  reset: () => void;
}

export const useWorkflowWorkspaceStore = create<WorkflowWorkspaceState>((set) => ({
  setWorkspace: (workspace) => set({ workspace, loadedAt: Date.now() }),
  invalidate: () => set({ loadedAt: undefined }),
  reset: () => set({ workspace: undefined, loadedAt: undefined }),
}));
