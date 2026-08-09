export interface RuntimeActivityStatus {
  activeTaskCount: number;
  productionBusy: boolean;
}

export interface DiagnosticsSummary {
  appVersion: string;
  platform: string;
  architecture: string;
  runMode: string;
  databaseHealthy: boolean;
  comfyStatus: "CONNECTED" | "OFFLINE" | "INCOMPATIBLE";
  comfyVersion?: string;
  gpuName?: string;
  vramTotal?: number;
  vramFree?: number;
  workflowPackages: number;
  validWorkflowPackages: number;
  invalidWorkflowPackages: number;
  activeTaskCount: number;
  productionBusy: boolean;
  loggingAvailable: boolean;
  logRetentionDays: number;
}

export interface DiagnosticsExport {
  fileName: string;
}
