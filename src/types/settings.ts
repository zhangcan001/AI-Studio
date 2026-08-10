export interface ComfySettingsView {
  schemaVersion: number;
  endpoint: string;
  warning?: string | null;
}

export interface ComfyEndpointTest {
  connected: boolean;
  endpoint: string;
  version?: string | null;
  gpu: string[];
  vramTotal?: number | null;
  vramFree?: number | null;
  nodeCount: number;
}

export interface RuntimeParameterProfile {
  id: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  values: Record<string, number>;
  updatedAt: string;
}
