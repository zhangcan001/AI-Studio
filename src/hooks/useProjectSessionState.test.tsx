// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useProjectSessionState } from "./useProjectSessionState";
afterEach(() => { cleanup(); sessionStorage.clear(); vi.restoreAllMocks(); });
const valid = (v: unknown): v is string => typeof v === "string";
it("restores navigation on remount and never leaks another project's preference", () => {
  const first = renderHook(({ project }) => useProjectSessionState(project, "search", "", valid), { initialProps: { project: "a" } });
  act(() => first.result.current[1]("find-a"));
  first.rerender({ project: "b" }); expect(first.result.current[0]).toBe("");
  act(() => first.result.current[1]("find-b"));
  first.rerender({ project: "a" }); expect(first.result.current[0]).toBe("find-a"); first.unmount();
  const next = renderHook(() => useProjectSessionState("a", "search", "", valid)); expect(next.result.current[0]).toBe("find-a");
});
it("rejects malformed preferences and works when WebView storage is unavailable", () => {
  sessionStorage.setItem(JSON.stringify(["ai-studio.navigation.v1", "a", "search"]), "[broken");
  const first = renderHook(() => useProjectSessionState("a", "search", "", valid)); expect(first.result.current[0]).toBe(""); first.unmount();
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw Error("unavailable"); });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw Error("unavailable"); });
  const next = renderHook(() => useProjectSessionState("a", "search", "", valid)); act(() => next.result.current[1]("still works")); expect(next.result.current[0]).toBe("still works");
});
