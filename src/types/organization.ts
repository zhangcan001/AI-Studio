import type { GenerationValues } from "./generation";
import type { ProjectView } from "./project";

export interface AssetTag {
  id: string;
  projectId: string;
  name: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectTemplate {
  id: string;
  name: string;
  description?: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
  available: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface TemplateProjectResult {
  project: ProjectView;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}
