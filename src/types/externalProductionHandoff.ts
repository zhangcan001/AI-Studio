export type ExternalProductionHandoffStage = {
  workflowVersionId: string;
  recipeId: string;
};

export type ExternalProductionHandoffDocument = {
  schemaVersion: 1;
  projectId: string;
  source: {
    agent: string;
    revision?: string;
  };
  series: Array<{
    externalId: string;
    name: string;
    description: string;
    ordinal: number;
    episodes: Array<{
      externalId: string;
      name: string;
      description: string;
      ordinal: number;
      scenes: Array<{
        externalId: string;
        name: string;
        description: string;
        ordinal: number;
        shots: Array<{
          externalId: string;
          name: string;
          ordinal: number;
          description: string;
          imagePrompt?: string;
          videoPrompt?: string;
          assetRefs?: Array<{ assetId: string }>;
          stages?: {
            image?: ExternalProductionHandoffStage;
            video?: ExternalProductionHandoffStage;
          };
        }>;
      }>;
    }>;
  }>;
  limits?: {
    maxShots?: number;
    maxSeries?: number;
    maxEpisodes?: number;
    maxScenes?: number;
  };
};

export type ExternalProductionHandoffIssue = {
  severity: string;
  code: string;
  message: string;
  path?: string;
};

export type ExternalProductionHandoffMapping = {
  entityKind: "series" | "episode" | "scene" | "shot";
  externalId: string;
  formalEntityId: string;
};

export type ExternalProductionHandoffReplay = {
  status: string;
  handoffId?: string;
  mappings: ExternalProductionHandoffMapping[];
};

export type ExternalProductionHandoffPreview = {
  projectId: string;
  documentSha256: string;
  seriesCount: number;
  episodeCount: number;
  sceneCount: number;
  shotCount: number;
  normalized: ExternalProductionHandoffDocument;
  errors: ExternalProductionHandoffIssue[];
  warnings: ExternalProductionHandoffIssue[];
  replay: ExternalProductionHandoffReplay;
  writePlan: {
    createsSeries: number;
    createsEpisodes: number;
    createsScenes: number;
    createsShots: number;
    writesPrompts: number;
    writesAssetReferences: number;
    writesStageConfigs: number;
    createsProvenanceMappings: number;
  };
};

export type ExternalProductionHandoffConfirmResult = {
  projectId: string;
  handoffId: string;
  documentSha256: string;
  replayed: boolean;
  seriesCount: number;
  episodeCount: number;
  sceneCount: number;
  shotCount: number;
  mappings: ExternalProductionHandoffMapping[];
};

export type ExternalProductionHandoffHistoryItem = {
  id: string;
  projectId: string;
  schemaVersion: number;
  sourceAgent: string;
  sourceRevision?: string;
  documentSha256: string;
  importedAt: string;
};
