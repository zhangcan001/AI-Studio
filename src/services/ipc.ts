import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export interface IpcErrorPayload {
  code: string;
  message: string;
  details?: unknown;
}

export class IpcError extends Error {
  readonly code: string;
  readonly details?: unknown;

  constructor(payload: IpcErrorPayload) {
    super(payload.message);
    this.name = "IpcError";
    this.code = payload.code;
    this.details = payload.details;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function codeFromMessage(message: string): string | undefined {
  return message.match(/^[A-Z][A-Z0-9_]{2,}/)?.[0];
}

function stringifyUnknown(error: unknown): string {
  if (typeof error === "string") return error;
  if (error === undefined) return "Unknown IPC error";
  try {
    const serialized = JSON.stringify(error);
    return serialized === undefined ? String(error) : serialized;
  } catch {
    return String(error);
  }
}

function parseStructuredPayload(error: unknown): IpcErrorPayload | undefined {
  if (!isRecord(error)) return undefined;

  const code = typeof error.code === "string" && error.code.trim() ? error.code : undefined;
  const message = typeof error.message === "string" ? error.message : undefined;
  if (!code || message === undefined) return undefined;

  return {
    code,
    message,
    ...(Object.prototype.hasOwnProperty.call(error, "details") ? { details: error.details } : {}),
  };
}

export function normalizeIpcError(error: unknown): IpcError {
  if (error instanceof IpcError) return error;

  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as unknown;
      const payload = parseStructuredPayload(parsed);
      if (payload) return new IpcError(payload);
    } catch {
      // Legacy string errors are handled below.
    }
  }

  const payload = parseStructuredPayload(error);
  if (payload) return new IpcError(payload);

  const message = error instanceof Error ? error.message : stringifyUnknown(error);
  const record = isRecord(error) ? error : undefined;
  const code = typeof record?.code === "string" && record.code.trim()
    ? record.code
    : codeFromMessage(message) ?? "UNKNOWN";
  const details = record && Object.prototype.hasOwnProperty.call(record, "details") ? record.details : undefined;

  return new IpcError({
    code,
    message,
    ...(details !== undefined ? { details } : {}),
  });
}

export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return args === undefined
      ? await tauriInvoke<T>(command)
      : await tauriInvoke<T>(command, args);
  } catch (error: unknown) {
    throw normalizeIpcError(error);
  }
}
