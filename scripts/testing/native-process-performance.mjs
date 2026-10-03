// Real process starts, not Page.reload. Uses only an explicitly owned fixture.
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { performance } from 'node:perf_hooks';
import { connectNative } from './native-cdp-session.mjs';
import { summarizeSamples } from './runtime-performance.mjs';

const [output, operation = 'startup'] = process.argv.slice(2);
if (!['startup', 'resume'].includes(operation)) throw Error('Unknown process measurement operation');
const root = resolve(process.env.AI_STUDIO_DATA_ROOT ?? '');
const expected = resolve(process.env.TEMP, 'ai-studio-phase12-native');
if (!output || root !== expected || process.env.AI_STUDIO_PERF_ISOLATED !== 'YES' ||
    process.env.AI_STUDIO_PERF_FRONTEND_MODE !== 'PRODUCTION_BUNDLE') {
  throw Error('Explicit owned Phase12 fixture and production bundle required');
}
const executable = resolve('src-tauri/target/debug/ai-studio.exe');
const powershell = (script) => execFileSync('powershell.exe',
  ['-NoProfile', '-NonInteractive', '-Command', script], { encoding: 'utf8', windowsHide: true });
if (powershell('@(Get-Process ai-studio -ErrorAction SilentlyContinue).Count').trim() !== '0') {
  throw Error('An app is already running; never close an unowned process');
}
const result = {
  phase: 12, type: operation === 'startup' ? 'REAL_PROCESS_STARTUP' : 'REAL_PROCESS_RESUME',
  sourceHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  sourceWorktreeDirty: Boolean(execFileSync('git', ['status', '--porcelain', '--', 'src', 'src-tauri/src'], { encoding: 'utf8' }).trim()),
  frontendBundleIndexSha256: createHash('sha256').update(readFileSync('dist/index.html')).digest('hex'),
  backendSourceHead: process.env.AI_STUDIO_PERF_BACKEND_SOURCE_HEAD ?? 'NOT_RECORDED',
  backendExecutableSha256: createHash('sha256').update(readFileSync(executable)).digest('hex'),
  dataset: { rootKind: 'owned isolated Phase11 fixture copy / ai-studio-phase12-native',
    inventory: JSON.parse(execFileSync('python', ['-c', `
import sqlite3,json,sys
from pathlib import Path
c=sqlite3.connect(Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True)
print(json.dumps({t:c.execute('SELECT count(*) FROM '+t).fetchone()[0] for t in ['tasks','assets','prompt_entries']}))
c.close()
`, join(root, 'app.db')], { encoding: 'utf8', windowsHide: true })) },
  buildMode: 'DEBUG', frontendMode: 'PRODUCTION_BUNDLE', osColdCacheControlled: false,
  timingBoundary: 'OS spawn request -> actual window handle + visible canonical route + observed IPC settled',
  samplePolicy: { warmup: 1, measured: 5 }, groups: {},
  limitations: ['Process-cold only, OS caches not flushed',
    'CDP startup IPC is captured only after debugger attachment; early calls may be missed',
    'Database migration and task recovery log timestamps are stage timings, not all-service readiness',
    'Debug backend / production frontend, not release binary latency'],
};
function stageTimings(launchUtcMs) {
  const lines = readdirSync(join(root, 'logs')).filter(n => n.startsWith('ai-studio.'))
    .flatMap(n => readFileSync(join(root, 'logs', n), 'utf8').split('\n'));
  const stages = {};
  for (const [key, text] of Object.entries({
    applicationStart: 'application starting', databaseReady: 'database migration completed',
    librarySynced: 'runtime workflow library synchronized', recoveryComplete: 'startup task recovery completed',
  })) {
    const stamps = lines.filter(l => l.includes(text)).map(l => Date.parse(l.split(' ')[0]))
      .filter(stamp => stamp >= launchUtcMs);
    stages[`${key}Ms`] = stamps.length ? Math.min(...stamps) - launchUtcMs : null;
  }
  return stages;
}
async function closeOwned(child) {
  if (child.exitCode !== null) return;
  // Both PID ownership and executable identity are checked. Graceful close only.
  const escaped = executable.replaceAll("'", "''");
  const closed = powershell(`$p=Get-Process -Id ${child.pid} -ErrorAction Stop;
    if($p.Path -ne '${escaped}'){throw 'Owned executable mismatch'};
    if(-not $p.CloseMainWindow()){throw 'No owned window; refusing termination'};
    if(-not $p.WaitForExit(20000)){throw 'Owned app did not exit gracefully'}; 'CLOSED'`).trim();
  if (closed !== 'CLOSED') throw Error('Owned process closure not verified');
}
function launchOwned() {
  const child = spawn(executable, [], { cwd: process.cwd(), windowsHide: true,
    stdio: 'ignore', env: { ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: join(process.env.TEMP, 'ai-studio-phase12-webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9224',
    } });
  child.on('error', error => { result.launchError = error.message; });
  writeFileSync(`${output}.owned.json`, JSON.stringify({ pid: child.pid, executable, launchUtcMs: Date.now() }));
  return child;
}
const resumeExpression = "JSON.parse(localStorage.getItem('aistudio.appRoute.v2'))?.route";
async function seedDeepLocator(mode) {
  const child = launchOwned();
  let session;
  try {
    session = await connectNative(); await session.settle('.v3-shell');
    const create = mode === 'create-shot', runs = mode === 'runs-run';
    await session.click(create ? '创作' : runs ? '运行' : '素材库');
    await session.settle(create ? '.create-page' : runs ? '.runs-page' : '.library-page');
    if (create) {
      await session.evaluate(`(()=>{const s=document.querySelector('select[aria-label="镜头"]');
        const o=[...s.options].find(o=>o.value);if(!o)throw Error('Fixture shot missing');
        s.value=o.value;s.dispatchEvent(new Event('change',{bubbles:true}))})()`);
    } else {
      await session.evaluate(`(()=>{const b=document.querySelector(${JSON.stringify(runs ? '.run-list button' : '.library-card')});
        if(!b)throw Error('Fixture deep child missing');b.click()})()`);
    }
    const selector = create ? '.create-content' : runs ? '.run-detail[aria-label="运行详情"]' : '.library-detail[aria-label="资源详情"]';
    await session.settle(selector);
    const route = await session.evaluate(resumeExpression);
    if (!(create ? route.shotId : runs ? route.run : route.resource)) throw Error('Deep locator not persisted by UI');
    return { selector, routePredicate: `JSON.stringify(${resumeExpression})===${JSON.stringify(JSON.stringify(route))}`,
      childKind: create ? 'shot' : runs ? 'run' : 'resource' };
  } finally { session?.close(); await closeOwned(child); }
}
try {
  for (const mode of operation === 'startup' ? ['process-cold', 'warm-process'] : ['create-shot', 'runs-run', 'library-resource']) {
    const target = operation === 'resume' ? await seedDeepLocator(mode) : {
      selector: '.v3-overview', routePredicate: `${resumeExpression}?.kind==='project'`, childKind: null,
    };
    const samples = [];
    for (let index = 0; index < 6; index++) {
      if (powershell('@(Get-Process ai-studio -ErrorAction SilentlyContinue).Count').trim() !== '0') {
        throw Error('Previous app still running');
      }
      const launchUtcMs = Date.now(), start = performance.now();
      const child = launchOwned();
      let session;
      writeFileSync(`${output}.owned.json`, JSON.stringify({ pid: child.pid, executable, launchUtcMs }));
      try {
        session = await connectNative();
        const attachedMs = session.attachedAtMs - start;
        await session.settle(target.selector, target.routePredicate);
        const visibleMs = performance.now() - start;
        const handle = powershell(`(Get-Process -Id ${child.pid}).MainWindowHandle.ToInt64()`).trim();
        if (handle === '0' || !/^\d+$/.test(handle)) throw Error('Actual native window not verified');
        const sample = { elapsedMs: performance.now() - start, visibleMs, debuggerAttachedMs: attachedMs,
          actualNativeWindow: true, canonicalRoute: mode, childKind: target.childKind,
          exactPersistedLocatorPreserved: true,
          ...stageTimings(launchUtcMs), startupIpc: session.snapshot(),
          startupIpcCoverage: 'ATTACHMENT_ONWARD_ONLY', windowVerificationCompletedMs: performance.now() - start,
        };
        if (index > 0) samples.push(sample);
        console.log(JSON.stringify({ mode, warmup: index === 0, elapsedMs: sample.elapsedMs,
          visibleMs, databaseReadyMs: sample.databaseReadyMs,
          observedIpc: sample.startupIpc.realIpc }));
      } finally {
        session?.close();
        await closeOwned(child);
      }
    }
    result.groups[mode] = { warmupCount: 1, ...summarizeSamples(samples.map(s => s.elapsedMs)),
      visible: summarizeSamples(samples.map(s => s.visibleMs)), raw: samples,
      primaryTiming: 'visible; elapsedMs includes external window-handle verification overhead' };
    writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
  }
} catch (error) {
  result.error = error.message; throw error;
} finally {
  writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
}
