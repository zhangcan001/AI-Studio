// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useLibraryController } from "../features/library/LibraryController";
import { useRunsController } from "../features/runs/RunsController";
import { invalidateRuns } from "./runInvalidation";
import type { LibraryList } from "./libraryTypes";
import type { RunList } from "./types";
import type { LibraryRoute } from "../features/library/libraryModel";
import type { RunsRoute } from "../features/runs/runsModel";

const api = vi.hoisted(() => ({
  runs: { list: vi.fn(), get: vi.fn(), resultsGet: vi.fn() },
  library: { list: vi.fn(), get: vi.fn(), relationsGet: vi.fn(), versionsGet: vi.fn(), tagsList: vi.fn() },
}));
vi.mock("./client", () => ({ productClient: { run: api.runs, library: api.library } }));

const emptyRuns: RunList = { items: [], nextCursor: null, coverage: "FINITE" };
const emptyLibrary: LibraryList = { items: [], nextCursor: null, coverage: "keyset-page", coverageMessage: "fixture" };

beforeEach(() => {
  vi.resetAllMocks();
  api.runs.list.mockResolvedValue(emptyRuns);
  api.runs.resultsGet.mockResolvedValue([]);
  api.library.list.mockResolvedValue(emptyLibrary);
  api.library.tagsList.mockResolvedValue([]);
  api.library.relationsGet.mockResolvedValue([]);
  api.library.versionsGet.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it("generate/start style invalidateRuns refreshes Runs without Library thrash", async () => {
  const navigate = vi.fn();
  const runsRoute: RunsRoute = { kind: "runs", projectId: "a" };
  const libraryRoute: LibraryRoute = { kind: "library", projectId: "a" };
  renderHook(() => useRunsController({ route: runsRoute, navigate }));
  renderHook(() => useLibraryController({ route: libraryRoute, navigate }));
  await act(async () => {});
  const runCalls = api.runs.list.mock.calls.length;
  const libraryCalls = api.library.list.mock.calls.length;
  vi.useFakeTimers();
  act(() => invalidateRuns("a", { library: false }));
  await act(async () => { vi.advanceTimersByTime(200); });
  expect(api.runs.list.mock.calls.length).toBe(runCalls + 1);
  expect(api.library.list.mock.calls.length).toBe(libraryCalls);
  act(() => invalidateRuns("a"));
  await act(async () => { vi.advanceTimersByTime(200); });
  expect(api.library.list.mock.calls.length).toBe(libraryCalls + 1);
});
