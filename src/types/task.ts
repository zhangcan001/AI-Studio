export type TaskStatus =
  | "CREATED"
  | "VALIDATING"
  | "PREPARING"
  | "QUEUED"
  | "RUNNING"
  | "COLLECTING"
  | "SUCCEEDED"
  | "FAILED";

export interface TaskProgress {
  mode: "indeterminate" | "node" | "step";
  current?: number;
  total?: number;
}

export interface TaskError {
  code: string;
  message: string;
}

export interface TaskView {
  id: string;
  status: TaskStatus;
  promptId?: string;
  queueNumber?: number;
  progress: TaskProgress;
  error?: TaskError;
  createdAt: string;
  queuedAt?: string;
  startedAt?: string;
  finishedAt?: string;
  outputAssetIds: string[];
}
