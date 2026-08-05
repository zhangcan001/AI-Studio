import { invoke } from "@tauri-apps/api/core";
import type { AppStatus } from "../types/app";
import type { CapabilitySummary, ComfyStatus } from "../types/comfy";

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
