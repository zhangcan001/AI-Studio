// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import type { RecipeViewModel } from '../../types/generation';
import { WorkflowBenchmarkPanel } from './WorkflowBenchmarkPanel';

const api = vi.hoisted(() => ({ presets: vi.fn(async () => []), list: vi.fn(async () => []) }));
vi.mock('../../services/workflowLabClient', async () => ({
  ...await vi.importActual<typeof import('../../services/workflowLabClient')>('../../services/workflowLabClient'),
  listPresets: api.presets, listWorkflowBenchmarks: api.list,
}));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it('phase12_benchmark bounds preset reads by exact references while preserving project and remount freshness', async () => {
  const catalog: RecipeViewModel[] = ['a', 'b'].map(id => ({ workflowId: `workflow_${id}`,
    workflowVersionId: `version_${id}`, recipeId: `recipe_${id}`, name: id,
    category: 'image', mode: 'T2I', fields: [], outputTypes: ['image'] }));
  const props = { projectId: 'project_a', catalog, baseValues: {}, baseReady: false };
  const view = render(<WorkflowBenchmarkPanel {...props} />);
  await act(async () => {});
  expect(api.presets.mock.calls).toEqual([
    ['project_a', 'version_a', 'recipe_a'], ['project_a', 'version_b', 'recipe_b'],
  ]);
  fireEvent.change(screen.getAllByLabelText('候选标签')[0], { target: { value: 'renamed' } });
  await act(async () => {});
  expect(api.presets).toHaveBeenCalledTimes(2);
  view.rerender(<WorkflowBenchmarkPanel {...props} projectId="project_b" />);
  await act(async () => {});
  expect(api.presets).toHaveBeenCalledTimes(4);
  expect(api.presets.mock.calls.slice(2)).toEqual([
    ['project_b', 'version_a', 'recipe_a'], ['project_b', 'version_b', 'recipe_b'],
  ]);
  view.unmount();
  render(<WorkflowBenchmarkPanel {...props} projectId="project_b" />);
  await act(async () => {});
  expect(api.presets).toHaveBeenCalledTimes(6);
  expect(api.list).toHaveBeenCalledTimes(3);
});

it('phase12_benchmark refetches changed exact version sets even when recipe names match', async () => {
  const catalog: RecipeViewModel[] = ['a', 'b'].map(id => ({ workflowId: `workflow_${id}`,
    workflowVersionId: `version_${id}`, recipeId: 'shared_recipe', name: 'same display name',
    category: 'image', mode: 'T2I', fields: [], outputTypes: ['image'] }));
  render(<WorkflowBenchmarkPanel projectId="project_a" catalog={catalog} baseValues={{}} baseReady={false} />);
  await act(async () => {});
  expect(api.presets.mock.calls).toEqual([
    ['project_a', 'version_a', 'shared_recipe'], ['project_a', 'version_b', 'shared_recipe'],
  ]);
  fireEvent.change(screen.getAllByLabelText('工作流 / 配方')[0], { target: { value: 'version_b:shared_recipe' } });
  await act(async () => {});
  expect(api.presets).toHaveBeenCalledTimes(3);
  expect(api.presets.mock.calls[2]).toEqual(['project_a', 'version_b', 'shared_recipe']);
  fireEvent.change(screen.getAllByLabelText('工作流 / 配方')[0], { target: { value: 'version_a:shared_recipe' } });
  await act(async () => {});
  expect(api.presets.mock.calls.slice(3)).toEqual([
    ['project_a', 'version_a', 'shared_recipe'], ['project_a', 'version_b', 'shared_recipe'],
  ]);
});
