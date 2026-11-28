export interface ComfySettingsView {
  schemaVersion: number;
  endpoint: string;
  authConfigured?: boolean;
  allowSystemProxy?: boolean;
  warning?: string | null;
}

export interface DesktopPreferences {
  autoFreeVramBetweenItems: boolean;
  transientRetryLimit: number;
  desktopNotifications: boolean;
  onboardingDismissed: boolean;
}

export interface ProjectStorageUsageView {
  projectId: string;
  projectName: string;
  sizeBytes: number;
}

export interface StorageSummaryView {
  currentDataRoot: string;
  defaultDataRoot: string;
  persistedCustomRoot?: string | null;
  pendingMigrationTarget?: string | null;
  envOverrideActive: boolean;
  freeBytes?: number | null;
  totalBytes?: number | null;
  databaseBytes: number;
  logsBytes: number;
  projectsTotalBytes: number;
  projects: ProjectStorageUsageView[];
}

export interface RejectedCleanupResultView {
  inspected: number;
  deletedCount: number;
  skippedInUse: number;
  freedBytes: number;
}

export interface ComfyEnvironmentProfile {
  id: string;
  name: string;
  endpoint: string;
  authHeader?: string | null;
  allowSystemProxy?: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface DataRootMigrationView {
  targetPath: string;
  migratedFiles: number;
  migratedBytes: number;
  backupRetainedPath: string;
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
  /** Public runtime/schema fingerprint, not proof of plugin compatibility. */
  environmentFingerprint?: string | null;
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
