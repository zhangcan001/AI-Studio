export interface ProductionScene {
  id: string;
  episodeId: string;
  ordinal: number;
  name: string;
  description: string;
  shotIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ProductionEpisode {
  id: string;
  seriesId: string;
  ordinal: number;
  name: string;
  description: string;
  scenes: ProductionScene[];
  createdAt: string;
  updatedAt: string;
}

export interface ProductionSeries {
  id: string;
  projectId: string;
  ordinal: number;
  name: string;
  description: string;
  episodes: ProductionEpisode[];
  createdAt: string;
  updatedAt: string;
}

export interface ProductionStructureTree {
  projectId: string;
  series: ProductionSeries[];
  unassignedShotIds: string[];
}

export interface ProductionSeriesRequest {
  projectId: string;
  seriesId?: string;
  name: string;
  description?: string;
}

export interface ProductionEpisodeRequest {
  projectId: string;
  seriesId: string;
  episodeId?: string;
  name: string;
  description?: string;
}

export interface ProductionSceneRequest {
  projectId: string;
  episodeId: string;
  sceneId?: string;
  name: string;
  description?: string;
}

export interface ProductionReorderRequest {
  projectId: string;
  parentId?: string;
  orderedIds: string[];
}

export interface ProductionAssignShotsRequest {
  projectId: string;
  sceneId: string;
  shotIds: string[];
}

export interface ProductionUnassignShotsRequest {
  projectId: string;
  shotIds: string[];
}

export interface ProductionSceneShotReorderRequest {
  projectId: string;
  sceneId: string;
  orderedShotIds: string[];
}
