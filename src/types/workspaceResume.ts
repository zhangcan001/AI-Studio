import type { ProjectView } from "./project";

export const WORKSPACES = [
  "command-center",
  "studio",
  "video",
  "shots",
  "assets",
  "tasks",
  "projects",
  "workflows",
  "settings",
] as const;

export type Workspace = (typeof WORKSPACES)[number];

export const DEFAULT_WORKSPACE: Workspace = "command-center";

export interface WorkspaceResume {
  lastProjectId?: string | null;
  lastWorkspace?: string | null;
  lastShotId?: string | null;
}

export interface WorkspaceNavigation {
  projectId?: string;
  workspace: Workspace;
  shotId?: string;
}

export function isWorkspace(value: unknown): value is Workspace {
  return typeof value === "string" && (WORKSPACES as readonly string[]).includes(value);
}

export function resolveWorkspaceNavigation(
  projects: Pick<ProjectView, "id">[],
  resume: WorkspaceResume | null | undefined,
  shotIds?: readonly string[],
): WorkspaceNavigation {
  const projectId = resume?.lastProjectId ?? undefined;
  if (!projectId || !projects.some((project) => project.id === projectId)) {
    return { workspace: DEFAULT_WORKSPACE };
  }

  const workspace = resume?.lastWorkspace;
  if (!isWorkspace(workspace) || workspace === DEFAULT_WORKSPACE) {
    return { projectId, workspace: DEFAULT_WORKSPACE };
  }

  if (workspace === "shots") {
    const shotId = resume?.lastShotId ?? undefined;
    if (!shotId || !shotIds?.includes(shotId)) {
      return { projectId, workspace: DEFAULT_WORKSPACE };
    }
    return { projectId, workspace, shotId };
  }

  return { projectId, workspace };
}
