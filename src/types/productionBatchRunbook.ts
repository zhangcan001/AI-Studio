import type { ShotStage } from "./shot";

export type ProductionBatchRunbookStage = ShotStage;
export type ProductionBatchRunbookStatus = "READY" | "RUNNING" | "PAUSED" | "COMPLETED" | string;
export type ProductionBatchRunbookFilter = "active" | "ready" | "running" | "paused" | "completed" | "all";

export interface ProductionBatchRunbookRequest {
  projectId: string;
  seriesId?: string | null;
}

export interface ProductionBatchRunbookRow {
  batchId: string;
  batchName: string;
  batchStatus: ProductionBatchRunbookStatus;
  stage?: ProductionBatchRunbookStage | null;
  seriesId?: string | null;
  seriesName?: string | null;
  seriesOrdinal?: number | null;
  episodeId?: string | null;
  episodeName?: string | null;
  episodeOrdinal?: number | null;
  sceneId?: string | null;
  sceneName?: string | null;
  sceneOrdinal?: number | null;
  shotCount: number;
  pending: number;
  active: number;
  succeeded: number;
  failed: number;
  cancelled?: number;
  skipped?: number;
  createdAt: string;
  readyToStart: boolean;
  blockedReason?: string | null;
  mixedScope?: boolean;
}

export interface ProductionBatchRunbookView {
  projectId: string;
  seriesId?: string | null;
  rows: ProductionBatchRunbookRow[];
  summary?: {
    batchTotal: number;
    readyBatches: number;
    runningBatches: number;
    pausedBatches: number;
    completedBatches: number;
    pending: number;
    active: number;
    succeeded: number;
    failed: number;
  };
  warnings?: Array<{ code: string; batchId: string; message: string }>;
  recommendedBatchId?: string | null;
  recommendationReason?: string | null;
}

export interface ProductionBatchRunbookPanelProps {
  projectId: string;
  runbook: ProductionBatchRunbookView;
  onRefresh?: () => Promise<void>;
  onStartBatch?: (batchId: string) => Promise<void>;
  onOpenProductionQueue?: (batchId?: string) => void;
  onNavigateToScene?: (sceneId: string) => void;
  onNavigateToEpisode?: (episodeId: string) => void;
}
