export type ToolHealthStatus = "AVAILABLE" | "MISSING" | "UNKNOWN";

export interface ToolView {
  id: string;
  name: string;
  type: string;
  description: string;
  metadata: unknown;
  createdAt: string;
}

export interface ToolInstanceView {
  id: string;
  toolId: string;
  path?: string | null;
  endpoint?: string | null;
  status: ToolHealthStatus;
  lastChecked?: string | null;
}

export interface ToolVersionView {
  id: string;
  toolId: string;
  version: string;
  observedAt: string;
  metadata: unknown;
}

export interface ToolCapabilityView {
  toolId: string;
  capabilityName: string;
  metadata: unknown;
}
