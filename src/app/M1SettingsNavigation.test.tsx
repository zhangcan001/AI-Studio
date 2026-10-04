// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { useAppRoute } from './routes/useAppRoute';
import { useStudioStore } from '../stores/studioStore';
import { routeProjectId, type AppRoute } from './routes/types';
import { appRouteReducer, initialRouteState } from './routes/reducer';
afterEach(() => { cleanup(); useStudioStore.getState().resetDraft(); localStorage.clear(); });
it('canonical Settings/Tool Hub return retains the exact Create route and existing saved draft', () => {
  const create = { kind: 'create', projectId: 'p', shotId: 's', stage: 'video', surface: 'batch' } as const;
  const store = useStudioStore.getState(); store.setValue('prompt', { type: 'string', value: 'owned unsaved draft' });
  store.setValue('first_frame', { type: 'image_asset', assetId: 'owned-image' });
  store.setCreationLabReturn({ scope: 'p:s:video', selectionRef: 'exact-selection', runRef: { source: 'queue-batch', id: 'owned-run' }, accepted: null });
  const before = useStudioStore.getState();
  const { result } = renderHook(() => useAppRoute());
  act(() => result.current.restore(create));
  const settings = { kind: 'system-settings', section: 'general', returnTo: create } as const;
  act(() => result.current.navigate(settings));
  act(() => result.current.navigate({ kind: 'system-settings', section: 'advanced-tools', returnTo: settings }));
  expect(routeProjectId(result.current.route)).toBe('p');
  act(() => result.current.back()); expect(result.current.route).toEqual(settings);
  act(() => result.current.back()); expect(result.current.route).toEqual(create);
  expect(useStudioStore.getState().values).toEqual(before.values);
  expect(useStudioStore.getState().draftDirty).toBe(true);
  expect(useStudioStore.getState().creationLabReturn).toEqual(before.creationLabReturn);
});
it('a resumed Tool Hub returns to the exact Settings return chain without another history owner', () => {
  const settings = { kind: 'system-settings', section: 'general', returnTo: { kind: 'project', projectId: 'p', page: 'overview' } } as const;
  let state = appRouteReducer(initialRouteState, { type: 'restore', route: { kind: 'system-settings', section: 'advanced-tools', returnTo: settings } });
  state = appRouteReducer(state, { type: 'back' }); expect(state.current).toEqual(settings);
  state = appRouteReducer(state, { type: 'back' }); expect(state.current).toEqual(settings.returnTo);
});
it('project generator destinations remain scoped canonical routes', () => {
  const route: AppRoute = { kind: 'project-settings', projectId: 'p', section: 'generators' };
  const state = appRouteReducer(initialRouteState, { type: 'navigate', route });
  expect(state.current).toEqual(route); expect(routeProjectId(state.current)).toBe('p');
});
