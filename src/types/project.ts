export interface ProjectView {
  id: string;
  name: string;
  description?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface RestoredProjectView extends ProjectView {
  status: string;
  backupVersion: number;
  assets: number;
  versions: number;
  generations: number;
  warnings: string[];
  missingTools: string[];
  missingModels: string[];
  missingFiles: string[];
  restoredGenerationToolUsages: number;
  restoredGenerationAssetVersions: number;
  unresolvedModelVersionIds: string[];
  unresolvedToolInstanceIds: string[];
  unresolvedToolVersionIds: string[];
}

export interface ProjectBackupExportView {
  fileName: string;
  bytes: number;
  entries: number;
  activeTasksExcluded: number;
}

export interface ProjectBackupPreview {
  inspectionId: string;
  projectName: string;
  imageCount: number;
  videoCount: number;
  audioCount: number;
  historyTasks: number;
  presets: number;
  productionQueues: number;
  benchmarks?: number;
  productionRuns?: number;
  promptEntries: number;
  shots?: number;
  assetVersions?: number;
  assetRelations?: number;
  models?: number;
  modelVersions?: number;
  tools?: number;
  toolInstances?: number;
  generationToolUsages?: number;
  generationAssetVersions?: number;
  missingWorkflows: string[];
  activeTasksExcluded: number;
  warning: string;
}
