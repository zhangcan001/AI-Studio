export interface ProjectView {
  id: string;
  name: string;
  description?: string | null;
  createdAt: string;
  updatedAt: string;
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
  promptEntries: number;
  shots?: number;
  missingWorkflows: string[];
  activeTasksExcluded: number;
  warning: string;
}
