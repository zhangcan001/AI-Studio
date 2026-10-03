// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useRunsController } from '../features/runs/RunsController';
import { useLibraryController } from '../features/library/LibraryController';
import { invalidateRuns } from '../product/runInvalidation';
import type { RunList, ProductRun } from '../product/types';
import type { LibraryList } from '../product/libraryTypes';
import type { RunsRoute } from '../features/runs/runsModel';
import type { LibraryRoute } from '../features/library/libraryModel';

const api = vi.hoisted(() => ({ runs: { list: vi.fn(), get: vi.fn(), resultsGet: vi.fn() }, library: { list: vi.fn(), get: vi.fn(), relationsGet: vi.fn(), versionsGet: vi.fn() } }));
vi.mock('../product/client', () => ({ productClient: { run: api.runs, library: api.library } }));
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r; }); return { promise, resolve }; }
const emptyRuns: RunList = { items: [], nextCursor: null, coverage: 'FINITE' };
const emptyLibrary: LibraryList = { items: [], nextCursor: null, coverage: 'keyset-page', coverageMessage: 'fixture' };
beforeEach(() => {
  vi.resetAllMocks(); api.runs.list.mockResolvedValue(emptyRuns); api.runs.resultsGet.mockResolvedValue([]);
  api.library.list.mockResolvedValue(emptyLibrary); api.library.relationsGet.mockResolvedValue([]);
  api.library.versionsGet.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it('phase11_runs rejects late project and route responses without issuing stale detail reads', async () => {
  const old = deferred<RunList>();
  api.runs.list.mockImplementation((project: string) => project === 'a' ? old.promise : Promise.resolve(emptyRuns));
  const navigate = vi.fn(); const route: RunsRoute = { kind: 'runs', projectId: 'a', run: { source: 'task', id: 'old' } };
  const hook = renderHook(({ route }) => useRunsController({ route, navigate }), { initialProps: { route } });
  await act(async () => hook.rerender({ route: { kind: 'runs', projectId: 'b', filter: 'completed' } }));
  expect(hook.result.current.loading).toBe(false); expect(hook.result.current.filter).toBe('completed');
  await act(async () => old.resolve({ ...emptyRuns, coverage: 'STALE' }));
  expect(hook.result.current.list).toEqual(emptyRuns); expect(api.runs.get).not.toHaveBeenCalled();
  expect(navigate).not.toHaveBeenCalled();
  const pending = deferred<ProductRun>(); api.runs.get.mockReturnValue(pending.promise);
  await act(async () => hook.rerender({ route: { kind: 'runs', projectId: 'b', run: { source: 'task', id: 'old' } } }));
  await act(async () => hook.rerender({ route: { kind: 'runs', projectId: 'b', filter: 'active' } }));
  await act(async () => pending.resolve({ ref: { source: 'task', id: 'old' } } as ProductRun));
  expect(hook.result.current.detail).toBeUndefined(); expect(hook.result.current.filter).toBe('active');
});

it('phase11_runs coalesces only own-project invalidations and disposes listeners timers and pending responses', async () => {
  const navigate = vi.fn(); const route: RunsRoute = { kind: 'runs', projectId: 'a' };
  const hook = renderHook(() => useRunsController({ route, navigate }));
  await act(async () => {}); expect(api.runs.list).toHaveBeenCalledTimes(1);
  vi.useFakeTimers();
  act(() => { invalidateRuns('b'); invalidateRuns('a'); invalidateRuns('a'); });
  expect(api.runs.list).toHaveBeenCalledTimes(1);
  await act(async () => vi.advanceTimersByTime(200)); expect(api.runs.list).toHaveBeenCalledTimes(2);
  const pending = deferred<RunList>(); api.runs.list.mockReturnValue(pending.promise);
  let refresh!: Promise<void>; act(() => { refresh = hook.result.current.refresh(); });
  act(() => invalidateRuns('a')); const calls = api.runs.list.mock.calls.length;
  hook.unmount();
  await act(async () => { pending.resolve({ ...emptyRuns, coverage: 'STALE' }); await refresh; vi.advanceTimersByTime(6000); invalidateRuns('a'); vi.advanceTimersByTime(200); });
  expect(api.runs.list).toHaveBeenCalledTimes(calls); expect(vi.getTimerCount()).toBe(0); expect(navigate).not.toHaveBeenCalled();
});

it('phase11_library abandons old projections and removes subscriptions on project switch and unmount', async () => {
  const old = deferred<LibraryList>(); api.library.list.mockImplementation((p: string) => p === 'a' ? old.promise : Promise.resolve(emptyLibrary));
  const navigate = vi.fn(); const route: LibraryRoute = { kind: 'library', projectId: 'a', resource: { kind: 'asset', id: 'old' } };
  const hook = renderHook(({ route }) => useLibraryController({ route, navigate }), { initialProps: { route } });
  await act(async () => hook.rerender({ route: { kind: 'library', projectId: 'b', filter: 'prompts' } }));
  expect(hook.result.current.loading).toBe(false); expect(hook.result.current.list).toEqual(emptyLibrary);
  await act(async () => old.resolve({ ...emptyLibrary, coverageMessage: 'STALE' }));
  expect(hook.result.current.list).toEqual(emptyLibrary); expect(api.library.get).not.toHaveBeenCalled();
  vi.useFakeTimers(); const calls = api.library.list.mock.calls.length;
  act(() => invalidateRuns('a')); await act(async () => vi.advanceTimersByTime(150)); expect(api.library.list).toHaveBeenCalledTimes(calls);
  act(() => { invalidateRuns('b'); invalidateRuns('b'); }); await act(async () => vi.advanceTimersByTime(150)); expect(api.library.list).toHaveBeenCalledTimes(calls + 1);
  act(() => invalidateRuns('b')); hook.unmount();
  await act(async () => { vi.advanceTimersByTime(6000); invalidateRuns('b'); vi.advanceTimersByTime(150); });
  expect(api.library.list).toHaveBeenCalledTimes(calls + 1); expect(vi.getTimerCount()).toBe(0); expect(navigate).not.toHaveBeenCalled();
});
