import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { IpcError, invokeCommand, normalizeIpcError } from "./ipc";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockedInvoke = vi.mocked(invoke);

describe("typed IPC transport", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  it("preserves structured code, message, and details", () => {
    const error = normalizeIpcError({
      code: "INVALID_INPUT",
      message: "technical message",
      details: { field: "name" },
    });

    expect(error).toBeInstanceOf(IpcError);
    expect(error.code).toBe("INVALID_INPUT");
    expect(error.message).toBe("technical message");
    expect(error.details).toEqual({ field: "name" });
  });

  it("preserves unknown structured codes and legacy strings", () => {
    expect(normalizeIpcError({ code: "NEW_BACKEND_CODE", message: "technical" })).toMatchObject({
      code: "NEW_BACKEND_CODE",
      message: "technical",
    });
    expect(normalizeIpcError("INVALID_INPUT: legacy failure")).toMatchObject({
      code: "INVALID_INPUT",
      message: "INVALID_INPUT: legacy failure",
    });
  });

  it("normalizes Error objects without throwing", () => {
    expect(normalizeIpcError(new Error("transport failed"))).toMatchObject({
      code: "UNKNOWN",
      message: "transport failed",
    });
  });

  it("keeps command names and argument shapes while normalizing rejection", async () => {
    mockedInvoke.mockRejectedValue({
      code: "PRODUCTION_QUEUE_BUSY",
      message: "queue is busy",
      details: { batchId: "batch-1" },
    });

    await expect(invokeCommand("production_queue_start", { projectId: "project-1" })).rejects.toMatchObject({
      code: "PRODUCTION_QUEUE_BUSY",
      message: "queue is busy",
      details: { batchId: "batch-1" },
    });
    expect(mockedInvoke).toHaveBeenCalledWith("production_queue_start", { projectId: "project-1" });
  });
});
