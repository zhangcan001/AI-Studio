import { beforeEach, afterEach } from "vitest";
import { setImmediate } from "node:timers";

// Historical proof tests perform long synchronous replay. Give the worker RPC
// channel an event-loop turn AND await real task-update acknowledgements before
// blocking again. A single afterEach turn can leave the next test's start update
// pending throughout a long synchronous replay. Never change RPC deadlines or
// turn rejected updates into success.
export function trackTaskUpdateRpc(rpc) {
  const pending = new Set();
  return {
    rpc: new Proxy(rpc, {
      get(target, key) {
        const value = Reflect.get(target, key);
        if (key !== "onTaskUpdate") return value;
        return (...args) => {
          const result = Reflect.apply(value, target, args);
          pending.add(result);
          // Retain rejections: flush must surface the original RPC failure.
          result.then(() => pending.delete(result), () => {});
          return result;
        };
      },
    }),
    async flush() {
      await new Promise(resolve => setImmediate(resolve));
      while (pending.size) await Promise.all([...pending]);
    },
  };
}

// Vitest 3's worker state owns the actual reporting RPC. Fail visibly if an
// upgrade changes this seam, rather than silently dropping the barrier.
const worker = globalThis.__vitest_worker__;
if (!worker?.rpc) throw new Error("Boundary RPC barrier requires Vitest worker RPC");
const tracked = trackTaskUpdateRpc(worker.rpc);
worker.rpc = tracked.rpc;
beforeEach(tracked.flush);
afterEach(tracked.flush);
