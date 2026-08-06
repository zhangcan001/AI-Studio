import type { DraftValue } from "./generation";

export interface PresetView {
  id: string;
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  values: Record<string, DraftValue>;
  createdAt: string;
  updatedAt: string;
}
