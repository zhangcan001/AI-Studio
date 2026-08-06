import { invoke } from "@tauri-apps/api/core";
import type { AppStatus } from "../types/app";
import type { CapabilitySummary, ComfyStatus } from "../types/comfy";
import type { AssetView } from "../types/asset";
import type { GenerationValues, RecipeViewModel } from "../types/generation";
import type { TaskView } from "../types/task";

export function ping(): Promise<string> {
  return invoke<string>("ping");
}

export function getAppStatus(): Promise<AppStatus> {
  return invoke<AppStatus>("get_app_status");
}

export function getComfyStatus(): Promise<ComfyStatus> {
  return invoke<ComfyStatus>("comfy_get_status");
}

export function refreshComfyCapabilities(): Promise<CapabilitySummary> {
  return invoke<CapabilitySummary>("comfy_refresh_capabilities");
}

export interface WorkflowSyncReport {
  packagesFound: number;
  valid: number;
  invalid: number;
  inserted: number;
  reused: number;
  errors: Array<{ package: string; code: string; message: string }>;
}

export function refreshWorkflowLibrary(): Promise<WorkflowSyncReport> {
  return invoke<WorkflowSyncReport>("workflow_library_refresh");
}

export function listGenerationCatalog(): Promise<RecipeViewModel[]> {
  return invoke<RecipeViewModel[]>("generation_catalog_list");
}

export function createGeneration(request: {
  projectId: string;
  workflowVersionId: string;
  recipeId: string;
  values: GenerationValues;
}): Promise<TaskView> {
  return invoke<TaskView>("generation_create", { request });
}

export function getTask(taskId: string): Promise<TaskView> {
  return invoke<TaskView>("task_get", { taskId });
}

export function listRecentTasks(limit = 10): Promise<TaskView[]> {
  return invoke<TaskView[]>("task_list_recent", { limit });
}

export interface RecoveryReport {
  examined: number;
  succeeded: number;
  failed: number;
  deferred: number;
  unresolved: number;
}

export function cancelTask(taskId: string): Promise<TaskView> {
  return invoke<TaskView>("task_cancel", { taskId });
}

export function reconcileActiveTasks(): Promise<RecoveryReport> {
  return invoke<RecoveryReport>("task_reconcile_active");
}

export function listAssetsByTask(taskId: string): Promise<AssetView[]> {
  return invoke<AssetView[]>("asset_list_by_task", { taskId });
}

export function listRecentAssets(projectId: string, limit = 100): Promise<AssetView[]> {
  return invoke<AssetView[]>("asset_list_recent", { projectId, limit });
}

export function pickAndImportImage(projectId: string): Promise<AssetView | null> {
  return invoke<AssetView | null>("asset_pick_and_import_image", { projectId });
}

export function readAssetImage(assetId: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("asset_read_image", { assetId });
}
