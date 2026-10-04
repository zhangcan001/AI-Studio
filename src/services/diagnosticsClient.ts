/** Advanced/System read seam. Not part of the normal Product use-case surface. */
import {
  getDiagnosticsSummary,
  getDiagnosticsExecutionHealth,
  getDiagnosticsRecentFailures,
  getDiagnosticsTaskTimeline,
  exportDiagnostics,
  repairJobsStatus,
} from "./tauriClient";

export const diagnosticsClient = {
  summary: getDiagnosticsSummary,
  executionHealth: getDiagnosticsExecutionHealth,
  recentFailures: getDiagnosticsRecentFailures,
  taskTimeline: getDiagnosticsTaskTimeline,
  exportBundle: exportDiagnostics,
  repairJobs: repairJobsStatus,
};
