export type ShotStatus = "DRAFT" | "READY" | "RUNNING" | "COMPLETED" | "FAILED";

export interface ShotRecord {
  id: string;
  projectId: string;
  ordinal: number;
  name: string;
  promptRef?: string;
  inlinePrompt?: string;
  workflowVersionId?: string;
  recipeId?: string;
  referenceAssetIds: string[];
  selectedResultAssetId?: string;
  status: ShotStatus;
}

export function validateShotRecord(shot: ShotRecord): string[] {
  const errors: string[] = [];
  if (!shot.projectId.trim()) errors.push("projectId");
  if (!shot.id.trim()) errors.push("id");
  if (!Number.isInteger(shot.ordinal) || shot.ordinal < 0) errors.push("ordinal");
  if (!shot.name.trim()) errors.push("name");
  if (shot.promptRef && shot.inlinePrompt) errors.push("prompt");
  if (shot.workflowVersionId && !shot.recipeId || shot.recipeId && !shot.workflowVersionId) errors.push("workflow");
  if (shot.referenceAssetIds.some((assetId) => !assetId.trim())) errors.push("referenceAssetIds");
  return errors;
}

export function reorderShots(projectId: string, shots: readonly ShotRecord[], orderedIds: readonly string[]): ShotRecord[] {
  const scoped = shots.filter((shot) => shot.projectId === projectId);
  const byId = new Map(scoped.map((shot) => [shot.id, shot]));
  if (orderedIds.length !== scoped.length || new Set(orderedIds).size !== orderedIds.length || orderedIds.some((id) => !byId.has(id))) {
    return scoped.map((shot) => ({ ...shot }));
  }
  return orderedIds.map((id, ordinal) => ({ ...byId.get(id)!, ordinal }));
}

export function selectShotResult(shot: ShotRecord, assetId: string): ShotRecord | undefined {
  if (!shot.referenceAssetIds.includes(assetId) && !assetId.trim()) return undefined;
  return { ...shot, selectedResultAssetId: assetId, status: "COMPLETED" };
}
