import { expect, it } from 'vitest';
// @ts-expect-error Existing static guard helper, excluded from browser application.
import { performanceSuccessor } from '../../scripts/style-boundary-guard.mjs';

it('phase12_guard permits only measured existing source successors chained to the frozen parent', () => {
  const path = 'src/features/experiments/WorkflowBenchmarkPanel.tsx';
  const beforeHash = 'a'.repeat(64), afterHash = 'b'.repeat(64);
  const source = { [path]: beforeHash };
  const seam = { path, status: 'MEASURED_VERIFIED', sourceFreeze: { beforeHash, afterHash },
    baseline: { samples: 5 }, after: { samples: 5 } };
  expect(performanceSuccessor(source, { optimizations: [seam] })).toEqual({
    snapshot: { [path]: afterHash }, violations: [],
  });
  for (const invalid of [
    { ...seam, path: 'src/services/ipc.ts' },
    { ...seam, path: 'src/features/experiments/NewAuthority.ts' },
    { ...seam, sourceFreeze: { beforeHash: 'c'.repeat(64), afterHash } },
    { ...seam, baseline: { samples: 1 } },
    { ...seam, after: {} },
  ]) {
    const result = performanceSuccessor(source, { optimizations: [invalid] });
    expect(result.snapshot).toEqual(source);
    expect(result.violations).toHaveLength(1);
  }
  expect(performanceSuccessor(source, { optimizations: [seam, seam] }).violations).toHaveLength(1);
  expect(performanceSuccessor(source, { optimizations: [{ ...seam, status: 'STATIC_CANDIDATE' }] }).snapshot).toEqual(source);
  expect(source).toEqual({ [path]: beforeHash });
});
