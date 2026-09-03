export interface ComfySettingsView {
  schemaVersion: number;
  endpoint: string;
  warning?: string | null;
}

export interface ComfyEnvironmentProfile {
  id: string;
  name: string;
  endpoint: string;
  createdAt: string;
  updatedAt: string;
}

export type ComfyPreflightStatus = "READY" | "WARNING" | "BLOCKED";

export type ComfyPreflightIssueSeverity = "ERROR" | "WARNING" | "INFO";

export interface ComfyPreflightIssue {
  severity: ComfyPreflightIssueSeverity;
  code: string;
  title: string;
  detail: string;
  workflowId?: string | null;
  workflowVersionId?: string | null;
  missingNodes?: string[] | null;
  suggestedAction?: string | null;
}

export interface ComfyPreflightWorkflowItem {
  workflowId?: string | null;
  workflowVersionId?: string | null;
  name?: string | null;
  version?: string | null;
  status: "READY" | "BLOCKED" | "DISABLED" | string;
  missingNodes?: string[] | null;
  reason?: string | null;
}

export interface ComfyPreflightWorkflowSummary {
  workflowTotal: number;
  workflowReady: number;
  workflowBlocked: number;
  items?: ComfyPreflightWorkflowItem[];
}

export interface ComfyPreflightReport {
  endpoint: string;
  status: ComfyPreflightStatus;
  checkedAt: string;
  connection: "CONNECTED" | "OFFLINE" | "INCOMPATIBLE";
  comfyuiVersion?: string | null;
  pythonVersion?: string | null;
  gpu?: string | null;
  vramTotal?: number | null;
  vramFree?: number | null;
  nodeCount?: number | null;
  runtimeBusy: boolean;
  activeTaskCount: number;
  productionBusy: boolean;
  workflowSummary: ComfyPreflightWorkflowSummary;
  issues: ComfyPreflightIssue[];
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
