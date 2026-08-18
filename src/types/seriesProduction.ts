import type { GenerationValues } from "./generation";
import type { PromptEntryView } from "./prompt";
import type { ReferenceAnchorView } from "./referenceAnchor";
import type { BatchWorkflowPreset } from "./sceneProduction";
import type { ShotStage, ShotView } from "./shot";
import type { ProductionStructureTree } from "./productionStructure";

export type SeriesProductionStage = ShotStage;
export type SeriesProductionEpisodeClassification = "EMPTY" | "DONE" | "PREPARED" | "READY" | "PARTIAL" | "BLOCKED";
export type SeriesProductionFilter = "all" | "ready" | "blocked" | "prepared" | "done";
export type SeriesProductionPrepareStatus = "SUCCESS" | "NOOP" | "PARTIAL" | "BLOCKED";

export interface SeriesProductionEpisodePlan {
  episodeId: string;
  episodeName: string;
  episodeOrdinal: number;
  sceneTotal: number;
  shotTotal: number;
  done: number;
  prepared: number;
  eligible: number;
  blocked: number;
  classification: SeriesProductionEpisodeClassification;
  canPrepare: boolean;
  existingBatchIds: string[];
  blockingReasons: string[];
}

export interface SeriesProductionPlan {
  projectId: string;
  seriesId: string;
  seriesName: string;
  seriesOrdinal: number;
  stage: SeriesProductionStage;
  episodeTotal: number;
  sceneTotal: number;
  shotTotal: number;
  done: number;
  prepared: number;
  eligible: number;
  blocked: number;
  readyEpisodeCount: number;
  blockedEpisodeCount: number;
  completedEpisodeCount: number;
  canPrepareAll: boolean;
  episodes: SeriesProductionEpisodePlan[];
}

export interface SeriesProductionPlanRequest {
  projectId: string;
  seriesId: string;
  stage: SeriesProductionStage;
}

export interface SeriesProductionPrepareRequest extends SeriesProductionPlanRequest {
  episodeIds: string[];
  allowPartial: boolean;
}

export interface SeriesProductionPrepareResultRow {
  episodeId: string;
  episodeName: string;
  status: string;
  createdBatches: number;
  createdItems: number;
  alreadyPrepared?: boolean;
  skipped?: boolean;
  blockingReasons?: string[];
  batchIds?: string[];
  error?: string | null;
}

export interface SeriesProductionPrepareResult {
  projectId: string;
  seriesId: string;
  stage: SeriesProductionStage;
  status?: SeriesProductionPrepareStatus;
  requestedEpisodes: number;
  requestedScenes: number;
  createdBatches: number;
  createdItems: number;
  alreadyPreparedEpisodes: string[];
  skippedDoneEpisodes: string[];
  skippedEmptyEpisodes: string[];
  skippedBlockedEpisodes: string[];
  episodeResults: SeriesProductionPrepareResultRow[];
}

export interface SeriesPresetApplyRequest {
  projectId: string;
  seriesId: string;
  stage: SeriesProductionStage;
  episodeIds: string[];
  sceneIds: string[];
  shotIds: string[];
  presetId: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}

export interface SeriesPromptBulkRequest {
  projectId: string;
  seriesId: string;
  stage: SeriesProductionStage;
  episodeIds: string[];
  sceneIds: string[];
  shotIds: string[];
  promptEntryId: string;
  promptVersionId: string;
  contextAnchorIds: string[];
  customValues: Record<string, string>;
}

export interface SeriesPromptPreview {
  total: number;
  valid: number;
  invalid: number;
  samples?: Array<{ shotId: string; text: string; valid: boolean; error?: string }>;
}

export type SeriesProductionBusyAction =
  | "plan"
  | "preset-apply"
  | "prompt-preview"
  | "prompt-apply"
  | "prepare";

export interface SeriesProductionPanelProps {
  projectId: string;
  tree: ProductionStructureTree;
  shots: ShotView[];
  promptEntries?: PromptEntryView[];
  referenceAnchors?: ReferenceAnchorView[];
  initialPresets?: BatchWorkflowPreset[];
  initialPlan?: SeriesProductionPlan;
  onPlan?: (request: SeriesProductionPlanRequest) => Promise<SeriesProductionPlan>;
  onPrepare?: (request: SeriesProductionPrepareRequest) => Promise<SeriesProductionPrepareResult>;
  onApplyPreset?: (request: SeriesPresetApplyRequest) => Promise<void>;
  onPreviewPrompt?: (request: SeriesPromptBulkRequest) => Promise<SeriesPromptPreview>;
  onApplyPrompt?: (request: SeriesPromptBulkRequest) => Promise<void>;
  onRefresh?: () => Promise<void>;
  onNotice?: (message: string) => void;
  onError?: (message: string) => void;
  onOpenProductionQueue?: () => void;
  onOpenRunbook?: () => void;
  onNavigateToEpisode?: (episodeId: string) => void;
}
