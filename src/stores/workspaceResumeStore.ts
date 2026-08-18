import { create } from "zustand";
import { getWorkspaceResume, saveWorkspaceResume } from "../services/tauriClient";
import type { Workspace } from "../types/workspaceResume";
import type { WorkspaceResume } from "../types/workspaceResume";

export const EMPTY_WORKSPACE_RESUME: WorkspaceResume = {
  lastProjectId: null,
  lastWorkspace: null,
  lastShotId: null,
};

interface WorkspaceResumeState {
  resume: WorkspaceResume;
  loaded: boolean;
  saving: boolean;
  error?: string;
  load: () => Promise<WorkspaceResume>;
  recordProjectChange: (projectId: string, workspace?: Workspace) => Promise<void>;
  recordWorkspaceChange: (workspace: Workspace, projectId?: string) => Promise<void>;
  recordShotChange: (shotId?: string) => Promise<void>;
}

let saveQueue = Promise.resolve();

function normalizeResume(resume?: WorkspaceResume | null): WorkspaceResume {
  return {
    lastProjectId: resume?.lastProjectId ?? null,
    lastWorkspace: resume?.lastWorkspace ?? null,
    lastShotId: resume?.lastShotId ?? null,
  };
}

function sameResume(left: WorkspaceResume, right: WorkspaceResume): boolean {
  return left.lastProjectId === right.lastProjectId
    && left.lastWorkspace === right.lastWorkspace
    && left.lastShotId === right.lastShotId;
}

function persist(next: WorkspaceResume, set: (state: Partial<WorkspaceResumeState>) => void): Promise<void> {
  set({ resume: next, saving: true, error: undefined });
  const request = saveQueue.then(async () => {
    try {
      const saved = normalizeResume(await saveWorkspaceResume(next));
      set({ resume: saved, saving: false });
    } catch (error: unknown) {
      // Resume is convenience state; a settings write failure must not block navigation.
      set({ saving: false, error: error instanceof Error ? error.message : "工作区位置暂时无法保存。" });
    }
  });
  saveQueue = request.catch(() => undefined);
  return request;
}

export const useWorkspaceResumeStore = create<WorkspaceResumeState>((set, get) => ({
  resume: EMPTY_WORKSPACE_RESUME,
  loaded: false,
  saving: false,
  load: async () => {
    try {
      const resume = normalizeResume(await getWorkspaceResume());
      set({ resume, loaded: true, error: undefined });
      return resume;
    } catch (error: unknown) {
      set({ resume: EMPTY_WORKSPACE_RESUME, loaded: true, error: error instanceof Error ? error.message : "工作区位置暂时无法读取。" });
      return EMPTY_WORKSPACE_RESUME;
    }
  },
  recordProjectChange: async (projectId, workspace = "projects") => {
    const current = get().resume;
    const next = { ...current, lastProjectId: projectId, lastWorkspace: workspace, lastShotId: null };
    if (sameResume(current, next)) return;
    await persist(next, set);
  },
  recordWorkspaceChange: async (workspace, projectId) => {
    const current = get().resume;
    const next = {
      ...current,
      lastProjectId: projectId ?? current.lastProjectId ?? null,
      lastWorkspace: workspace,
    };
    if (sameResume(current, next)) return;
    await persist(next, set);
  },
  recordShotChange: async (shotId) => {
    const current = get().resume;
    const next = { ...current, lastShotId: shotId ?? null };
    if (sameResume(current, next)) return;
    await persist(next, set);
  },
}));
