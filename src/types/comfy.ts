export type ComfyConnectionStatus = "CONNECTED" | "OFFLINE" | "INCOMPATIBLE";

export interface ComfyDeviceInfo {
  name?: string;
  deviceType?: string;
  vramTotal?: number;
  vramFree?: number;
}

export interface ComfySystemSummary {
  pythonVersion?: string;
  os?: string;
  ramTotal?: number;
  ramFree?: number;
}

export interface CapabilitySummary {
  nodeCount: number;
  capturedAt: string;
}

export interface ComfyStatus {
  status: ComfyConnectionStatus;
  endpoint: string;
  comfyuiVersion?: string;
  system?: ComfySystemSummary;
  devices: ComfyDeviceInfo[];
  capability?: CapabilitySummary;
}
