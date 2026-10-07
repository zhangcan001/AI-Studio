// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useRuntimeStatusMonitor } from "./useRuntimeStatusMonitor";
const api = vi.hoisted(() => ({ getComfyStatus: vi.fn() }));
const status = { status: "CONNECTED", endpoint: "fixture", runtimeGeneration: 1, devices: [] };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.getComfyStatus.mockResolvedValue(status); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("uses the existing typed health authority on entry and every 10 seconds, not readiness polling", async () => {
  const publish = vi.fn(); renderHook(() => useRuntimeStatusMonitor(true, publish, api.getComfyStatus));
  await act(async () => {}); expect(publish).toHaveBeenLastCalledWith(status);
  await act(async () => { await vi.advanceTimersByTimeAsync(9_999); }); expect(api.getComfyStatus).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(1); }); expect(api.getComfyStatus).toHaveBeenCalledTimes(2);
});
it("serializes probes, and cancels late results/timers on leaving Create", async () => {
  let resolve!: (value: unknown) => void; api.getComfyStatus.mockReturnValueOnce(new Promise(yes => { resolve = yes; }));
  const publish = vi.fn(); const view = renderHook(({ enabled }) => useRuntimeStatusMonitor(enabled, publish, api.getComfyStatus), { initialProps: { enabled: true } });
  await act(async () => { await vi.advanceTimersByTimeAsync(30_000); }); expect(api.getComfyStatus).toHaveBeenCalledTimes(1);
  view.rerender({ enabled: false }); await act(async () => { resolve(status); await vi.advanceTimersByTimeAsync(30_000); });
  expect(publish).not.toHaveBeenCalled(); expect(api.getComfyStatus).toHaveBeenCalledTimes(1);
});
it("invalidates the UI precheck on typed-transport failure and keeps observing reconnect", async () => {
  api.getComfyStatus.mockRejectedValueOnce(new Error("IPC unavailable"));
  const publish = vi.fn(); renderHook(() => useRuntimeStatusMonitor(true, publish, api.getComfyStatus));
  await act(async () => {}); expect(publish).toHaveBeenLastCalledWith(undefined);
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); }); expect(publish).toHaveBeenLastCalledWith(status);
});
