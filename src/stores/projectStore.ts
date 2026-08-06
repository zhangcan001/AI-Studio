import { create } from "zustand";
import type { ProjectView } from "../types/project";

export const ACTIVE_PROJECT_STORAGE_KEY = "aistudio.activeProjectId";
export const DEFAULT_PROJECT_ID = "prj_default";

interface ProjectState {
  projects: ProjectView[];
  activeProjectId?: string;
  loading: boolean;
  error?: string;
  setProjects: (projects: ProjectView[]) => void;
  setActiveProject: (projectId: string) => void;
  upsertProject: (project: ProjectView) => void;
  setLoading: (loading: boolean) => void;
  setError: (error?: string) => void;
  activeProject: () => ProjectView | undefined;
}

function readStoredProjectId(): string | undefined {
  try {
    return typeof localStorage === "undefined"
      ? undefined
      : localStorage.getItem(ACTIVE_PROJECT_STORAGE_KEY) ?? undefined;
  } catch {
    return undefined;
  }
}

function persistProjectId(projectId?: string) {
  try {
    if (typeof localStorage === "undefined") return;
    if (projectId) localStorage.setItem(ACTIVE_PROJECT_STORAGE_KEY, projectId);
    else localStorage.removeItem(ACTIVE_PROJECT_STORAGE_KEY);
  } catch {
    // A storage failure must not block local project browsing.
  }
}

export function resolveActiveProjectId(
  projects: ProjectView[],
  savedProjectId = readStoredProjectId(),
): string | undefined {
  if (savedProjectId && projects.some((project) => project.id === savedProjectId)) {
    return savedProjectId;
  }
  if (projects.some((project) => project.id === DEFAULT_PROJECT_ID)) {
    return DEFAULT_PROJECT_ID;
  }
  return projects[0]?.id;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  loading: true,
  setProjects: (projects) => {
    const activeProjectId = resolveActiveProjectId(projects);
    persistProjectId(activeProjectId);
    set({ projects, activeProjectId, error: undefined });
  },
  setActiveProject: (activeProjectId) => {
    if (!get().projects.some((project) => project.id === activeProjectId)) return;
    persistProjectId(activeProjectId);
    set({ activeProjectId, error: undefined });
  },
  upsertProject: (project) =>
    set((state) => {
      const projects = [project, ...state.projects.filter((item) => item.id !== project.id)];
      const activeProjectId = state.activeProjectId && projects.some((item) => item.id === state.activeProjectId)
        ? state.activeProjectId
        : resolveActiveProjectId(projects);
      persistProjectId(activeProjectId);
      return { projects, activeProjectId, error: undefined };
    }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  activeProject: () => {
    const { projects, activeProjectId } = get();
    return projects.find((project) => project.id === activeProjectId);
  },
}));
