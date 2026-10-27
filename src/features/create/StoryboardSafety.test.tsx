// @vitest-environment jsdom
import { useState } from 'react';
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { StoryboardWorkbench } from './StoryboardWorkbench';
import { StoryboardShotRows } from './StoryboardShotRows';
import { RunsPage } from '../runs/RunsPage';
import { usePersistentVideoInputs } from './usePersistentVideoInputs';
import { useStudioStore } from '../../stores/studioStore';
import { invalidateVideoInputs, resetVideoInputInvalidationForTests } from '../../product/videoInputInvalidation';
import type { AppRoute } from '../../app/routes/types';
import type { CreationContext, GeneratorOption, ProductRun } from '../../product/types';
import type { StoryboardShot } from './storyboardModel';
import type { VideoInputView } from '../../types/shotVideoInput';
const api = vi.hoisted(() => ({ get: vi.fn(), workbench: vi.fn(), generatorsList: vi.fn(), createShot: vi.fn(), updateShot: vi.fn(), deleteShot: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), selectResult: vi.fn(), referencesSet: vi.fn(), inputsGet: vi.fn(), inputsSave: vi.fn(), assetsImport: vi.fn(), runList: vi.fn(), runGet: vi.fn(), resultsGet: vi.fn() }));
vi.mock('../../product/client', () => ({ productClient: { creation: { ...api, mediaUrl: () => '', workbench: api.workbench }, run: { list: api.runList, get: api.runGet, resultsGet: api.resultsGet }, storyEntities: { promptSections: vi.fn(async () => ({ style: null, summary: null, soundscape: null, music: null })), list: vi.fn(async () => []), listForShot: vi.fn(async () => []), bindShot: vi.fn(async () => []) }, library: { imageGet: vi.fn(async () => [1]), list: vi.fn(async () => ({ items: [], nextCursor: null })), tagsList: vi.fn(async () => []) } } }));
const generator: GeneratorOption = { selectionRef: 'g', name: 'Local', version: '1', mode: 'T2V', mediaKind: 'video', availability: true, availabilityReason: null, recommended: true, fields: [{ type: 'textarea', key: 'prompt', label: '提示词', required: true, default: '' }] };
const mediaGenerator: GeneratorOption = { ...generator, persistentInputs: true, mode: 'FIRST_LAST', fields: [{ type: 'image', key: 'first_frame', label: '首帧', required: true }, { type: 'image', key: 'last_frame', label: '尾帧', required: true }] };
let shots = [{ id: 'shot1', name: '镜头一', ordinal: 0 }, { id: 'shot2', name: '镜头二', ordinal: 1 }];
function workbenchFrom(shotFn: (id: string | null) => CreationContext = context, limit = 60) {
  const base = shotFn(null);
  const all = (base.shots.length ? base.shots : shots).map(s => {
    const ctx = shotFn(s.id);
    return { shot: ctx.selectedShot!, candidates: ctx.candidates };
  }).filter(item => item.shot);
  const truncated = Math.max(0, all.length - limit);
  return { projectId: 'p', projectName: 'P', stage: 'video' as const, shots: base.shots, items: all.slice(0, limit), truncated };
}
function context(id: string | null): CreationContext { const summary = shots.find(s => s.id === id); return { projectId: 'p', projectName: 'P', stage: 'video', shots, promptChoices: [], candidates: [], mediaInputs: [], selectedShot: summary ? { summary, prompt: `saved-${id}`, selectionRef: 'g', values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null } : null }; }
const run: ProductRun = { ref: { source: 'queue-batch', id: 'r' }, projectId: 'p', title: '历史运行', status: 'SUCCEEDED', phase: 'SUCCEEDED', createdAt: '2026-01-01', updatedAt: '2026-01-01', progress: { total: 1, succeeded: 1, failed: 0, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 0 }, resultsSummary: [], errorSummary: null, preferredParent: null, availableActions: [], detail: { sources: [{ id: 'shot1', name: '镜头一', stage: 'video' }], inputs: [{ taskId: 't', generatorName: 'Local', selectionRef: 'g', values: { prompt: { type: 'string', value: 'history-prompt' } }, reuseUnavailableReason: null, errorMessage: null }] } };
const route = { kind: 'create', projectId: 'p', shotId: 'shot1', stage: 'video' } as const;
const runtime = { status: 'CONNECTED', endpoint: 'http://fixture.invalid', runtimeGeneration: 1, devices: [] } as const;
const dirty = vi.fn(), navigate = vi.fn();
function Host({ deny = false, initialShot = 'shot1' }: {
    deny?: boolean;
    initialShot?: string;
}) { const [r, set] = useState<AppRoute>({ ...route, shotId: initialShot || undefined }); if (r.kind !== 'create')
    return <p>left</p>; return <StoryboardWorkbench route={r} runtime={{ ...runtime, devices: [] }} onDirtyChange={dirty} navigate={n => { navigate(n); if (deny)
    return false; set(n); return true; }} renderRuns={(r, n) => <RunsPage route={r} navigate={n}/>}/>; }
function view(revision = 1): VideoInputView { return { selectionRef: 'g', token: { instanceId: 'i', revision }, inputs: [{ inputKey: 'first_frame', ordinal: 0, assetId: 'saved-first' }, { inputKey: 'last_frame', ordinal: 0, assetId: 'saved-last' }], assets: [] }; }
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => resolve = r); return { promise, resolve }; }
beforeEach(() => { vi.resetAllMocks(); useStudioStore.getState().resetDraft(); resetVideoInputInvalidationForTests(); sessionStorage.clear(); localStorage.clear(); shots = [{ id: 'shot1', name: '镜头一', ordinal: 0 }, { id: 'shot2', name: '镜头二', ordinal: 1 }]; api.get.mockImplementation((_p, id) => Promise.resolve(context(id))); api.workbench.mockImplementation(async (_p?: string, _stage?: string, limit = 60) => workbenchFrom(context, limit)); api.generatorsList.mockResolvedValue([generator]); api.createShot.mockResolvedValue({ id: 'shot3', name: '新镜头', ordinal: 2 }); api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] }); api.runList.mockImplementation(async (_p, filter) => ({ items: filter === 'active' ? [] : [run], nextCursor: null, coverage: '' })); api.runGet.mockResolvedValue(run); api.resultsGet.mockResolvedValue([]); api.inputsGet.mockResolvedValue(view()); api.inputsSave.mockImplementation(async (r) => ({ ...view(2), inputs: r.inputs })); api.generate.mockResolvedValue({ accepted: true, runRef: run.ref, startOutcome: 'QUEUED', startIssue: null }); HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); }; URL.createObjectURL = vi.fn(() => 'blob:fixture'); URL.revokeObjectURL = vi.fn(); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
async function reuse() { fireEvent.click(screen.getByRole('button', { name: '任务队列与运行记录' })); const panel = await screen.findByRole('dialog', { name: '任务队列与运行记录' }); fireEvent.click(await within(panel).findByRole('button', { name: /历史运行.*已完成/ })); fireEvent.click(await within(panel).findByRole('button', { name: '编辑这些输入并创建新运行' })); await waitFor(() => expect(navigate).toHaveBeenCalled()); }
it('AUDIT-01 same-Shot historical input reuse applies immediately, not to a later Shot', async () => { render(<Host />); await screen.findByRole('textbox', { name: '提示词' }); await screen.findByRole('button', { name: '操作镜头二' }); await reuse(); await waitFor(() => expect(screen.queryByRole('dialog', { name: '任务队列与运行记录' })).toBeNull()); const current = useStudioStore.getState().values.prompt; fireEvent.click(screen.getByRole('button', { name: '操作镜头二' })); await waitFor(() => expect(screen.getByRole('button', { name: '操作镜头二' }).getAttribute('aria-current')).toBe('true')); await waitFor(() => expect(api.generatorsList).toHaveBeenCalledWith('p', 'video', 'shot2')); await waitFor(() => expect(useStudioStore.getState().pendingRunIntent).toBeUndefined()); expect(current).toEqual({ type: 'string', value: 'history-prompt' }); expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot2' }); });
it('AUDIT-01b declined navigation does not leave a reuse intent for a later Shot', async () => { render(<Host deny/>); await screen.findByRole('textbox', { name: '提示词' }); await reuse(); expect(screen.getByRole('dialog', { name: '任务队列与运行记录' })).toBeTruthy(); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined(); });
it('AUDIT-02 invalidation during input save releases the write lock after completion', async () => { const p = deferred<VideoInputView>(); api.inputsSave.mockReturnValueOnce(p.promise); const h = renderHook(() => usePersistentVideoInputs(route, mediaGenerator, false)); await waitFor(() => expect(h.result.current.ready).toBe(true)); act(() => useStudioStore.getState().setValue('first_frame', { type: 'image_asset', assetId: 'new-first' })); let saving!: Promise<void>; act(() => { saving = h.result.current.save(); }); await waitFor(() => expect(h.result.current.busy).toBe(true)); await act(async () => { invalidateVideoInputs('p'); }); expect(api.inputsGet).toHaveBeenCalledTimes(1); await act(async () => { p.resolve({ ...view(2), inputs: [{ inputKey: 'first_frame', ordinal: 0, assetId: 'new-first' }, { inputKey: 'last_frame', ordinal: 0, assetId: 'saved-last' }] }); await saving; }); expect(h.result.current.busy).toBe(false); await waitFor(() => expect(h.result.current.ready).toBe(true)); expect(h.result.current.view?.token?.revision).toBe(2); expect(useStudioStore.getState().values.first_frame).toEqual({ type: 'image_asset', assetId: 'new-first' }); });
it('AUDIT-03 background input invalidation does not silently discard an unsaved draft', async () => { const h = renderHook(() => usePersistentVideoInputs(route, mediaGenerator, false)); await waitFor(() => expect(h.result.current.ready).toBe(true)); act(() => useStudioStore.getState().setValue('first_frame', { type: 'image_asset', assetId: 'unsaved-first' })); expect(h.result.current.dirty).toBe(true); await act(async () => { invalidateVideoInputs('p'); }); await waitFor(() => expect(api.inputsGet).toHaveBeenCalledTimes(2)); expect(useStudioStore.getState().values.first_frame).toEqual({ type: 'image_asset', assetId: 'unsaved-first' }); });
it('AUDIT-04 lost generation acknowledgement reuses the original row submission key', async () => { api.generate.mockRejectedValueOnce(new Error('IPC response lost after persistence')); const summary = shots[0]; const item: StoryboardShot = { ...summary, shot: context('shot1').selectedShot, candidates: [], run: null, status: 'TODO' }; render(<StoryboardShotRows projectId='p' items={[item]} busy={false} onOpen={vi.fn()} onCreate={vi.fn()} onChanged={vi.fn()} onQueue={vi.fn()}/>); const button = await screen.findByRole('button', { name: '生成视频' }); fireEvent.click(button); await screen.findByRole('alert'); fireEvent.click(button); await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(2)); const keys = api.generate.mock.calls.map(c => c[0].submissionIdempotencyKey); expect(keys[1]).toBe(keys[0]); });
it('AUDIT-05 workbench count includes shots outside the bounded detail window', async () => { shots = Array.from({ length: 65 }, (_, i) => ({ id: `shot${i + 1}`, name: `镜头${i + 1}`, ordinal: i })); render(<StoryboardWorkbench route={{ kind: 'create', projectId: 'p', stage: 'video' }} navigate={vi.fn()}/>); await screen.findByText(/另有 5 个镜头未在此展开/); const stats = screen.getByLabelText('镜头统计'); expect(within(stats).getByText('镜头总数').nextSibling?.textContent).toBe('65'); });
it('AUDIT-06 a post-create context read failure does not freeze the retained workbench', async () => { api.get.mockImplementation((_p, id) => id === 'shot3' ? Promise.reject({ code: 'INVALID_INPUT', message: '新镜头读取失败' }) : Promise.resolve(context(id))); render(<Host initialShot=''/>); const add = await screen.findByRole('button', { name: '+ 新建镜头' }); await waitFor(() => expect(add.hasAttribute('disabled')).toBe(false)); fireEvent.click(add); await screen.findByRole('alert'); fireEvent.click(screen.getByRole('button', { name: '展开分镜列表' })); const again = await screen.findByRole('button', { name: '+ 新建镜头' }); expect(again.hasAttribute('disabled')).toBe(false); });
it('CONTROL healthy workbench reads do not submit or select results', async () => { render(<Host />); await screen.findByRole('textbox', { name: '提示词' }); await screen.findByRole('button', { name: '操作镜头二' }); expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' }); expect(api.generate).not.toHaveBeenCalled(); expect(api.selectResult).not.toHaveBeenCalled(); });
it('reusing another Run into the same Shot remains repeatable and never generates', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    await reuse();
    await waitFor(() => expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'history-prompt' }));
    api.runGet.mockResolvedValue({ ...run, detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], values: { prompt: { type: 'string', value: 'second-history' } } }] } });
    await reuse();
    await waitFor(() => expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'second-history' }));
    expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
    expect(api.generate).not.toHaveBeenCalled();
});
it('declining same-Shot dirty-draft replacement preserves the current draft and clears reuse', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    act(() => useStudioStore.getState().setValue('prompt', { type: 'string', value: 'my-dirty-draft' }));
    fireEvent.click(screen.getByRole('button', { name: '任务队列与运行记录' }));
    const panel = await screen.findByRole('dialog', { name: '任务队列与运行记录' });
    fireEvent.click(await within(panel).findByRole('button', { name: /历史运行.*已完成/ }));
    fireEvent.click(await within(panel).findByRole('button', { name: '编辑这些输入并创建新运行' }));
    await waitFor(() => expect(window.confirm).toHaveBeenCalled());
    await waitFor(() => expect(useStudioStore.getState().pendingRunIntent).toBeUndefined());
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'my-dirty-draft' });
    expect(navigate).not.toHaveBeenCalled();
    expect(api.generate).not.toHaveBeenCalled();
});
it('closing Runs while generator validation is pending never applies a late reuse intent', async () => {
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    fireEvent.click(screen.getByRole('button', { name: '任务队列与运行记录' }));
    const panel = await screen.findByRole('dialog', { name: '任务队列与运行记录' });
    fireEvent.click(await within(panel).findByRole('button', { name: /历史运行.*已完成/ }));
    const button = await within(panel).findByRole('button', { name: '编辑这些输入并创建新运行' });
    const validation = deferred<GeneratorOption[]>();
    api.generatorsList.mockReturnValueOnce(validation.promise);
    fireEvent.click(button);
    fireEvent.click(within(panel).getByRole('button', { name: '返回镜头操作' }));
    await act(async () => { validation.resolve([generator]); });
    expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' });
    expect(navigate).not.toHaveBeenCalled();
});
it('targeted reuse cannot apply to a different Shot or project, and cannot apply before navigation approval', async () => {
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    const intent = { projectId: 'p', stage: 'video' as const, selectionRef: 'g', values: { prompt: { type: 'string' as const, value: 'do-not-apply' } }, target: { shotId: 'shot1', requestId: 'one', approved: false } };
    act(() => useStudioStore.getState().setPendingRunIntent(intent));
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' });
    act(() => useStudioStore.getState().setPendingRunIntent({ ...intent, target: { ...intent.target, shotId: 'shot2', approved: true } }));
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' });
    act(() => useStudioStore.getState().setPendingRunIntent({ ...intent, projectId: 'other', target: { ...intent.target, approved: true } }));
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' });
    expect(api.generate).not.toHaveBeenCalled();
});
it('an unavailable targeted generator is rejected without silently replacing the current draft', async () => {
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    act(() => useStudioStore.getState().setPendingRunIntent({ projectId: 'p', stage: 'video', selectionRef: 'missing', values: { prompt: { type: 'string', value: 'blocked' } }, target: { shotId: 'shot1', requestId: 'unavailable', approved: true } }));
    await waitFor(() => expect(useStudioStore.getState().pendingRunIntent).toBeUndefined());
    expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'saved-shot1' });
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(api.generate).not.toHaveBeenCalled();
});
it('historical media enters only the intended dirty draft, not persisted bindings or a later Shot', async () => {
    api.generatorsList.mockResolvedValue([{ ...mediaGenerator, fields: [...generator.fields, ...mediaGenerator.fields] }]);
    api.runGet.mockResolvedValue({ ...run, detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], values: { prompt: { type: 'string', value: 'history-media' }, first_frame: { type: 'image_asset', assetId: 'history-first' }, last_frame: { type: 'image_asset', assetId: 'history-last' } } }] } });
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    await waitFor(() => expect(api.inputsGet).toHaveBeenCalled());
    await reuse();
    await waitFor(() => expect(useStudioStore.getState().values.first_frame).toEqual({ type: 'image_asset', assetId: 'history-first' }));
    expect(useStudioStore.getState().values.last_frame).toEqual({ type: 'image_asset', assetId: 'history-last' });
    expect(api.inputsSave).not.toHaveBeenCalled();
    expect(api.generate).not.toHaveBeenCalled();
});
it('mixed-batch history requires an explicit target Shot instead of guessing its first source', async () => {
    api.runGet.mockResolvedValue({ ...run, detail: { ...run.detail, sources: [{ id: 'shot1', name: '镜头一', stage: 'video' }, { id: 'shot2', name: '镜头二', stage: 'video' }] } });
    render(<Host />);
    await screen.findByRole('textbox', { name: '提示词' });
    await reuse();
    await screen.findByText(/请选择目标镜头；历史输入只载入/);
    const picker = await screen.findByRole('combobox', { name: '镜头' });
    expect((picker as HTMLSelectElement).value).toBe('');
    expect(useStudioStore.getState().pendingRunIntent?.target?.shotId).toBeNull();
    fireEvent.change(picker, { target: { value: 'shot2' } });
    await waitFor(() => expect(useStudioStore.getState().values.prompt).toEqual({ type: 'string', value: 'history-prompt' }));
    expect(screen.getByRole('button', { name: '操作镜头二' }).getAttribute('aria-current')).toBe('true');
    expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
    expect(api.generate).not.toHaveBeenCalled();
});
it('a rejected navigation rolls back only its own staged reuse intent', async () => {
    const navigation = deferred<boolean>();
    const rejectedNavigate = vi.fn(() => navigation.promise);
    render(<RunsPage route={{ kind: 'runs', projectId: 'p', run: run.ref }} navigate={rejectedNavigate}/>);
    fireEvent.click(await screen.findByRole('button', { name: '编辑这些输入并创建新运行' }));
    await waitFor(() => expect(rejectedNavigate).toHaveBeenCalled());
    expect(useStudioStore.getState().pendingRunIntent?.target?.approved).toBe(false);
    await act(async () => { navigation.resolve(false); });
    await waitFor(() => expect(useStudioStore.getState().pendingRunIntent).toBeUndefined());
    expect(api.generate).not.toHaveBeenCalled();
});
it('an older refused navigation never clears a newer pending operation', async () => {
    const navigation = deferred<boolean>();
    const delayedNavigate = vi.fn(() => navigation.promise);
    render(<RunsPage route={{ kind: 'runs', projectId: 'p', run: run.ref }} navigate={delayedNavigate}/>);
    fireEvent.click(await screen.findByRole('button', { name: '编辑这些输入并创建新运行' }));
    await waitFor(() => expect(delayedNavigate).toHaveBeenCalled());
    const newer = { projectId: 'p', stage: 'video' as const, selectionRef: 'g', values: {}, target: { shotId: 'shot2', requestId: 'newer', approved: false } };
    act(() => useStudioStore.getState().setPendingRunIntent(newer));
    await act(async () => { navigation.resolve(false); });
    expect(useStudioStore.getState().pendingRunIntent).toBe(newer);
});
it('rejected Shot creation releases its own busy flag and can be retried', async () => {
    api.createShot.mockRejectedValueOnce({ code: 'INVALID_INPUT' });
    render(<StoryboardWorkbench route={{ kind: 'create', projectId: 'p', stage: 'video' }} navigate={() => false}/>);
    const add = await screen.findByRole('button', { name: '+ 新建镜头' });
    await waitFor(() => expect(add.hasAttribute('disabled')).toBe(false));
    fireEvent.click(add);
    await screen.findByRole('alert');
    await waitFor(() => expect(add.hasAttribute('disabled')).toBe(false));
    fireEvent.click(add);
    await waitFor(() => expect(api.createShot).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(add.hasAttribute('disabled')).toBe(false));
});
