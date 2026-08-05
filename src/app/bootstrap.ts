import {
  getAppStatus,
  getComfyStatus,
  ping,
  refreshComfyCapabilities,
} from "../services/tauriClient";
import type { AppStatus } from "../types/app";
import type { ComfyStatus } from "../types/comfy";

export interface BootstrapState {
  ping: string;
  status: AppStatus;
  comfy: ComfyStatus;
}

export async function bootstrap(): Promise<BootstrapState> {
  if (!bootstrapPromise) {
    bootstrapPromise = bootstrapInternal().catch((error: unknown) => {
      bootstrapPromise = null;
      throw error;
    });
  }

  return bootstrapPromise;
}

let bootstrapPromise: Promise<BootstrapState> | null = null;

async function bootstrapInternal(): Promise<BootstrapState> {
  const [pingResponse, status, comfy] = await Promise.all([
    ping(),
    getAppStatus(),
    getComfyStatus(),
  ]);
  let initialComfy = comfy;

  if (comfy.status === "CONNECTED") {
    const capability = await refreshComfyCapabilities().catch(() => undefined);
    if (capability) {
      initialComfy = { ...comfy, capability };
    }
  }

  return {
    ping: pingResponse,
    status,
    comfy: initialComfy,
  };
}
