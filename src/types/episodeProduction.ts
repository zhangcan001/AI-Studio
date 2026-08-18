import type { PromptEntryView } from "./prompt";
import type { ReferenceAnchorView } from "./referenceAnchor";
import type { BatchWorkflowPreset } from "./sceneProduction";
import type { ShotStage, ShotView } from "./shot";
import type { ProductionStructureTree } from "./productionStructure";

export type EpisodeProductionStage = ShotStage;
export type EpisodeProductionSceneClassification = "DONE" | "PREPARED" | "READY" | "PARTIAL" | "BLOCKED" | "EMPTY";
export type EpisodeProductionFilter = "all" | "ready" | "prepared" | "blocked" | "done";
export type EpisodeProductionPrepareStatus = "SUCCESS" | "NOOP" | "PARTIAL" | "BLOCKED";

export interface EpisodeProductionScenePlan {
  sceneId: string;
  sceneName: string;
  sceneOrdinal: number;
  total: number;
  done: number;
  prepared: number;
  eligible: number;
  blocked: number;
  canPrepare: boolean;
  classification: EpisodeProductionSceneClassification;
  existingBatchIds: string[];
  blockingReasons: string[];
}

export interface EpisodeProductionPlan {
  projectId: string;
  seriesId: string;
  seriesName: string;
  episodeId: string;
  episodeName: string;
  episodeOrdinal: number;
  stage: EpisodeProductionStage;
  sceneTotal: number;
  shotTotal: number;
  done: number;
  prepared: number;
  eligible: number;
  blocked: number;
  readySceneCount: number;
  blockedSceneCount: number;
  fullyDoneSceneCount: number;
  canPrepareAll: boolean;
  scenes: EpisodeProductionScenePlan[];
}

export interface EpisodeProductionPlanRequest {
  projectId: string;
  episodeId: string;
  stage: EpisodeProductionStage;
}

export interface EpisodeProductionPrepareRequest extends EpisodeProductionPlanRequest {
  sceneIds: string[];
  allowPartial: boolean;
}

export interface EpisodeProductionPrepareResultRow {
  sceneId: string;
  sceneName: string;
  status: string;
  created: boolean;
  createdCount: number;
  batchId?: string | null;
  existingBatchIds: string[];
  blockingReasons: string[];
  error?: string | null;
}

export interface EpisodeProductionPrepareResult {
  projectId: string;
  episodeId: string;
  stage: EpisodeProductionStage;
  status?: EpisodeProductionPrepareStatus;
  requestedScenes: number;
  createdBatches: number;
  createdItems: number;
  alreadyPreparedScenes: string[];
  skippedDoneScenes: string[];
  skippedEmptyScenes: string[];
  skippedBlockedScenes: string[];
  results: EpisodeProductionPrepareResultRow[];
}

export interface EpisodeProductionPanelProps {
  projectId: string;
  tree: ProductionStructureTree;
  shots: ShotView[];
  promptEntries?: PromptEntryView[];
  referenceAnchors?: ReferenceAnchorView[];
  initialPresets?: BatchWorkflowPreset[];
  initialPlan?: EpisodeProductionPlan;
  onRefresh?: () => Promise<void>;
  onNotice?: (message: string) => void;
  onError?: (message: string) => void;
  onOpenProductionQueue?: () => void;
  onNavigateToScene?: (sceneId: string) => void;
}
