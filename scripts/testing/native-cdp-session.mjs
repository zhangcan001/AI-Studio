// Acceptance only; never imported by production. No bridge replacement.
import { performance } from 'node:perf_hooks';
import { createRuntimeMeasurement } from './runtime-performance.mjs';

export async function connectNative({ port = 9224, timeoutMs = 30000 } = {}) {
  const deadline = performance.now() + timeoutMs;
  let target;
  while (performance.now() < deadline) {
    try {
      const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`, {
        signal: AbortSignal.timeout(500),
      })).json();
      target = tabs.find(t => t.type === 'page' &&
        ['about:blank', 'http://localhost:1420/'].includes(t.url));
      if (target) break;
    } catch { /* The owned process has not exposed its debugger yet. */ }
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  if (!target) throw Error('Owned Native debugger deadline');
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let serial = 0;
  const pending = new Map();
  let observer = createRuntimeMeasurement();
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (message.method) observer.observe(message);
    const call = pending.get(message.id);
    if (!call) return;
    clearTimeout(call.timer); pending.delete(message.id);
    if (message.error || message.result?.exceptionDetails) {
      call.reject(Error(JSON.stringify(message.error ?? message.result.exceptionDetails)));
    } else call.resolve(message.result);
  };
  function cdp(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++serial;
      pending.set(id, { resolve, reject, timer: setTimeout(() => {
        pending.delete(id); reject(Error(`${method} deadline`));
      }, timeoutMs) });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  async function evaluate(expression) {
    return (await cdp('Runtime.evaluate', {
      expression, awaitPromise: true, returnByValue: true,
    })).result.value;
  }
  await cdp('Network.enable');
  return {
    cdp, evaluate, attachedAtMs: performance.now(), initialTargetUrl: target.url,
    reset() { observer = createRuntimeMeasurement(); },
    snapshot() { return observer.snapshot(); },
    async settle(selector, routePredicate = 'true') {
      const deadline = performance.now() + timeoutMs;
      while (performance.now() < deadline) {
        try {
          const visible = await evaluate(`!!document.querySelector(${JSON.stringify(selector)}) &&
            !document.querySelector('.workspace-loading,[aria-busy="true"]') &&
            ![...document.querySelectorAll('[role="alert"]')].some(e=>e.textContent.trim()) &&
            ![...document.querySelectorAll('[role="status"]')].some(e=>/正在读取|正在加载|正在载入/.test(e.textContent)) &&
            (${routePredicate})`);
          if (visible && !observer.pending) {
            await evaluate('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
            if (!observer.pending) return;
          }
        } catch { /* Initial navigation may replace the execution context. */ }
        await new Promise(resolve => setTimeout(resolve, 10));
      }
      throw Error(`Native visible completion deadline: ${selector}`);
    },
    async click(label) {
      await evaluate(`(()=>{const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(label)}&&!b.disabled);
        if(!b)throw Error('Required action missing');b.click()})()`);
    },
    close() {
      for (const call of pending.values()) {
        clearTimeout(call.timer); call.reject(Error('Acceptance session closed'));
      }
      pending.clear(); socket.close();
    },
  };
}
