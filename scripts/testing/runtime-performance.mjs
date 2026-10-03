// Measurement-only helpers. Imported by acceptance tools, never by the app.
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { classifyIpcRequest } from './native-ipc-observer.mjs';

export function summarizeSamples(values) {
  if (!values.length || values.some(value => !Number.isFinite(value) || value < 0)) {
    throw new Error('Measurements must be finite nonnegative samples');
  }
  const ordered = [...values].sort((a, b) => a - b);
  const middle = Math.floor(ordered.length / 2);
  return { samples: values.length, min: ordered[0], median: ordered.length % 2
    ? ordered[middle] : (ordered[middle - 1] + ordered[middle]) / 2,
  max: ordered.at(-1) };
}

export function createRuntimeMeasurement(now = () => performance.now()) {
  const requests = new Map();
  let preflights = 0, probes = 0;
  return {
    observe(event) {
      const entry = classifyIpcRequest(event);
      if (entry.kind === 'PREFLIGHT') preflights++;
      if (entry.kind === 'PROBE') probes++;
      if (entry.kind === 'COMMAND' && !requests.has(entry.requestId)) {
        // Only a digest leaves the process; no prompt, path, credential or body.
        const fingerprint = createHash('sha256').update(JSON.stringify(entry.args)).digest('hex');
        requests.set(entry.requestId, { command: entry.command, fingerprint,
          startedMs: now(), payloadBytes: Buffer.byteLength(event.params.request.postData),
          completedMs: null, responseBytes: null, failed: false });
      }
      if (['Network.loadingFinished', 'Network.loadingFailed'].includes(event.method)) {
        const request = requests.get(event.params.requestId);
        if (request && request.completedMs === null) {
          request.completedMs = now();
          request.failed = event.method === 'Network.loadingFailed';
          // CDP encoded bytes are transport size, NOT decoded DTO size.
          request.responseBytes = event.params.encodedDataLength ?? null;
        }
      }
    },
    get pending() { return [...requests.values()].filter(r => r.completedMs === null).length; },
    snapshot() {
      const entries = [...requests.values()];
      const byCommand = {}, repeated = {};
      for (const entry of entries) {
        byCommand[entry.command] = (byCommand[entry.command] ?? 0) + 1;
        const key = `${entry.command}:${entry.fingerprint}`;
        repeated[key] = (repeated[key] ?? 0) + 1;
      }
      return { realIpc: entries.length, uniqueCommands: Object.keys(byCommand).length,
        preflights, probes, pending: this.pending, byCommand,
        repeatedSemanticCalls: Object.values(repeated).reduce((n, count) => n + Math.max(0, count - 1), 0),
        duplicateClassification: 'CANDIDATE; requires action/retry/intent review',
        payloadBytes: entries.reduce((n, r) => n + r.payloadBytes, 0),
        responseBytes: entries.some(r => r.responseBytes === null) ? null
          : entries.reduce((n, r) => n + r.responseBytes, 0),
        calls: entries.map(({ command, startedMs, completedMs, payloadBytes, responseBytes, failed }) => ({
          command, elapsedMs: completedMs === null ? null : completedMs - startedMs,
          payloadBytes, responseBytes, failed,
        })) };
    },
  };
}

export async function measureScenario({ prepare, action, ready, sampleCount = 5, warmupCount = 1 }) {
  if (sampleCount < 5 || warmupCount < 1) throw new Error('Require one warmup and five measured runs');
  const samples = [];
  for (let index = 0; index < warmupCount + sampleCount; index++) {
    await prepare();
    const start = performance.now();
    await action();
    const observation = await ready();
    const elapsedMs = performance.now() - start;
    if (index >= warmupCount) samples.push({ elapsedMs, ...observation });
  }
  return { warmupCount, ...summarizeSamples(samples.map(sample => sample.elapsedMs)), raw: samples };
}
