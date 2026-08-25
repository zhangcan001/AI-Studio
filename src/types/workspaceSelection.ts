export type WorkspaceSelection =
  | { type: "project"; projectId: string }
  | { type: "series"; seriesId: string }
  | { type: "episode"; episodeId: string }
  | { type: "scene"; sceneId: string }
  | { type: "shot"; shotId: string };

export type WorkspaceSelectionType = WorkspaceSelection["type"];

export function workspaceSelectionKey(selection: WorkspaceSelection): string {
  switch (selection.type) {
    case "project":
      return `project:${selection.projectId}`;
    case "series":
      return `series:${selection.seriesId}`;
    case "episode":
      return `episode:${selection.episodeId}`;
    case "scene":
      return `scene:${selection.sceneId}`;
    case "shot":
      return `shot:${selection.shotId}`;
  }
}

export function isSameWorkspaceSelection(
  left: WorkspaceSelection | undefined,
  right: WorkspaceSelection | undefined,
): boolean {
  return left !== undefined && right !== undefined && workspaceSelectionKey(left) === workspaceSelectionKey(right);
}
