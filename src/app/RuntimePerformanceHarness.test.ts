import { expect, it } from 'vitest';
// @ts-expect-error Node measurement helper, excluded from application transport.
import { createRuntimeMeasurement, summarizeSamples } from '../../scripts/testing/runtime-performance.mjs';

it('phase12_measurement preserves attempts without exporting sensitive request bodies', () => {
  let clock = 0;
  const counter = createRuntimeMeasurement(() => clock);
  const request = (id: string, method = 'POST') => ({ method: 'Network.requestWillBeSent', params: {
    requestId: id, request: { url: 'http://ipc.localhost/product_run_list', method,
      postData: method === 'POST' ? JSON.stringify({ projectId: 'isolated', secret: 'not-for-evidence' }) : undefined },
  } });
  counter.observe(request('preflight', 'OPTIONS'));
  counter.observe(request('one')); counter.observe(request('one'));
  counter.observe(request('retry'));
  expect(counter.pending).toBe(2);
  clock = 5;
  counter.observe({ method: 'Network.loadingFinished', params: { requestId: 'one', encodedDataLength: 17 } });
  counter.observe({ method: 'Network.loadingFailed', params: { requestId: 'retry' } });
  const result = counter.snapshot();
  expect(result.realIpc).toBe(2); expect(result.preflights).toBe(1);
  expect(result.repeatedSemanticCalls).toBe(1); expect(result.pending).toBe(0);
  expect(result.responseBytes).toBeNull(); expect(result.calls[0].elapsedMs).toBe(5);
  expect(result.calls[1].failed).toBe(true);
  expect(JSON.stringify(result)).not.toContain('not-for-evidence');
});

it('phase12_measurement reports median rather than fastest or average sample', () => {
  const input = [90, 10, 40, 30, 20];
  expect(summarizeSamples(input)).toEqual({ samples: 5, min: 10, median: 30, max: 90 });
  expect(input).toEqual([90, 10, 40, 30, 20]);
  expect(summarizeSamples([1, 3])).toEqual({ samples: 2, min: 1, median: 2, max: 3 });
  expect(() => summarizeSamples([])).toThrow();
  expect(() => summarizeSamples([NaN])).toThrow();
});
