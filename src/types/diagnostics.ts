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
  comfyStatus: "CONNECTED" | "OFFLINE" | "INCOMPATIBLE" | "UNKNOWN";
  comfyVersion?: string;
  gpuName?: string;
  vramTotal?: number;
  vramFree?: number;
  workflowPackages: number | null;
  validWorkflowPackages: number | null;
  invalidWorkflowPackages: number | null;
  activeTaskCount: number | null;
  productionBusy: boolean | null;
  loggingAvailable: boolean;
  logRetentionDays: number;
}

export interface DiagnosticsExport {
  fileName: string;
}

export type TelemetryCompleteness = "COMPLETE" | "PARTIAL" | "LEGACY_UNAVAILABLE" | "INVALID";

export interface PhaseDurations {
  prepareMs: number | null;
  submitMs: number | null;
  queueWaitMs: number | null;
  executionMs: number | null;
  collectionMs: number | null;
  totalMs: number | null;
}

export interface DiagnosticTaskTimeline {
  taskId: string;
  projectId: string;
  status: string;
  completeness: TelemetryCompleteness;
  durations: PhaseDurations;
  createdAt: string;
  queuedAt: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  prepareStartedAt: string | null;
  preparedAt: string | null;
  submittedAt: string | null;
  executionStartedAt: string | null;
  executionFinishedAt: string | null;
  collectionFinishedAt: string | null;
  generationExecutionId: string | null;
  runtimeProfile: string | null;
  concurrencyClass: string | null;
}

export interface DiagnosticDurationSample {
  medianMs: number | null;
  sampleCount: number;
}

export interface DiagnosticExecutionHealth {
  projectId: string;
  windowLimit: number;
  sampleCount: number;
  active: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  telemetryComplete: number;
  telemetryPartial: number;
  telemetryUnavailable: number;
  telemetryInvalid: number;
  prepare: DiagnosticDurationSample;
  submit: DiagnosticDurationSample;
  queueWait: DiagnosticDurationSample;
  execution: DiagnosticDurationSample;
  collection: DiagnosticDurationSample;
  total: DiagnosticDurationSample;
}

export interface DiagnosticRecentFailure {
  code: string;
  status: string;
  count: number;
  affectedTaskCount: number;
  latestAt: string | null;
  projectId: string;
  exampleTaskId: string;
}
