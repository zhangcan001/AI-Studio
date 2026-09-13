export interface ModelView {
  id: string;
  name: string;
  provider: string;
  type: string;
  description: string;
  metadata: unknown;
  createdAt: string;
}

export interface ModelVersionView {
  id: string;
  modelId: string;
  version: string;
  capabilities: unknown;
  parameterSchema: unknown;
  createdAt: string;
}
