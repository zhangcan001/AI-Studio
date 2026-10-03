// Explicit isolated Native acceptance only. Not a CI timing threshold.
import { writeFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { createRuntimeMeasurement, measureScenario } from './runtime-performance.mjs';

const [port, output] = process.argv.slice(2);
if (port !== '9224' || !output || process.env.AI_STUDIO_PERF_ISOLATED !== 'YES') {
  throw new Error('Requires owned isolated app on port 9224, output path and AI_STUDIO_PERF_ISOLATED=YES');
}
const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const tab = tabs.find(tab => tab.url === 'http://localhost:1420/');
if (!tab) throw new Error('Owned development Native page not found');
const socket = new WebSocket(tab.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let serial = 0, observer = createRuntimeMeasurement();
const pending = new Map();
socket.onmessage = event => {
  const message = JSON.parse(event.data);
  if (message.method) observer.observe(message);
  const call = pending.get(message.id);
  if (!call) return;
  clearTimeout(call.timer); pending.delete(message.id);
  if (message.error || message.result?.exceptionDetails) call.reject(new Error(JSON.stringify(message.error ?? message.result.exceptionDetails)));
  else call.resolve(message.result);
};
function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++serial;
    pending.set(id, { resolve, reject, timer: setTimeout(() => { pending.delete(id); reject(new Error(`${method} deadline`)); }, 30000) });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  return (await cdp('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result.value;
}
async function settle(selector = '.v3-shell') {
  const deadline = performance.now() + 20000;
  while (performance.now() < deadline) {
    await evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    const visible = await evaluate(`!!document.querySelector(${JSON.stringify(selector)}) &&
      ![...document.querySelectorAll('[role="status"]')].some(e=>/正在读取|正在加载|正在载入/.test(e.textContent)) &&
      !document.querySelector('.workspace-loading,[aria-busy="true"]')`);
    if (visible && !observer.pending) {
      // Verify a second frame, not a fixed sleep used to mask a race.
      await evaluate('new Promise(resolve => requestAnimationFrame(resolve))');
      if (!observer.pending) return;
    }
  }
  throw new Error(`Visible completion not reached: ${selector}; pending=${observer.pending}`);
}
async function click(label) {
  await evaluate(`(()=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(label)}&&!b.disabled);
    if(!button)throw Error('Required navigation button missing');button.click()})()`);
}
async function prepare(label) { await click(label); await settle(); }
async function switchProject(projectId) {
  await evaluate(`(()=>{const selector=document.querySelector('.v3-project-picker select');
    if(![...selector.options].some(o=>o.value===${JSON.stringify(projectId)}))throw Error('Isolated project missing');
    selector.value=${JSON.stringify(projectId)};selector.dispatchEvent(new Event('change',{bubbles:true}))})()`);
}
const result = { phase: 12, type: 'NATIVE_BASELINE', buildMode: 'DEBUG',
  frontendMode: process.env.AI_STUDIO_PERF_FRONTEND_MODE ?? 'DEVELOPMENT_STRICT_EFFECTS',
  dataKind: 'owned isolated Phase11 fixture copy', sourceHead: '0bb137303bbcf4baa98a109e7558d5899ef236fc',
  timingBoundary: 'actual user action -> visible completion + actual IPC completion + animation frame',
  osColdCacheControlled: false, queryCount: 'NOT_MEASURED', renderCount: 'NOT_MEASURED',
  startup: 'NOT_MEASURED; no navigation/reload substituted for process launch',
  resume: 'NOT_MEASURED; requires separate actual process relaunch measurement',
  scenarios: {}, limitations: ['Encoded transport bytes are not decoded DTO size',
    'Repeated semantic calls require intent/retry review', 'No global heap-leak absence claim'] };
try {
  await cdp('Network.enable');
  await settle();
  const scenarios = [
    ['project-overview', () => prepare('运行'), () => click('概览'), '.v3-overview'],
    ['project-switch', async () => { await switchProject('prj_default'); await settle(); await prepare('概览'); },
      () => switchProject('prj_19ce69c5-f438-44a3-a0fb-92e41c569f98'), '.v3-overview'],
    ['create-ready', async () => { await switchProject('prj_default'); await settle(); await prepare('运行'); }, () => click('创作'), '.create-page'],
    ['runs-load', () => prepare('概览'), () => click('运行'), '.runs-page'],
    ['runs-refresh', () => prepare('运行'), () => click('刷新'), '.runs-page'],
    ['library-load', () => prepare('运行'), () => click('素材库'), '.library-page'],
    ['library-prompts', () => prepare('素材库'), () => click('提示词'), '.library-page'],
    ['workflow-lab', async () => { await prepare('系统设置'); }, () => click('高级工作流'), '.workflow-lab'],
  ];
  for (const [name, prepareAction, action, selector] of scenarios) {
    result.scenarios[name] = await measureScenario({
      prepare: async () => { await prepareAction(); observer = createRuntimeMeasurement(); },
      action,
      ready: async () => { await settle(selector); return { ipc: observer.snapshot() }; },
    });
    writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify({ scenario: name, ...result.scenarios[name], raw: undefined }));
  }
} finally {
  writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
  for (const call of pending.values()) clearTimeout(call.timer);
  socket.close();
}
