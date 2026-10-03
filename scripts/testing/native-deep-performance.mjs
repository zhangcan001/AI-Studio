// Isolated acceptance-only deep read/mutation and instrumented commit/lifetime samples.
import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import { connectNative } from './native-cdp-session.mjs';
import { measureScenario } from './runtime-performance.mjs';

const [output] = process.argv.slice(2);
const dataRoot = resolve(process.env.AI_STUDIO_DATA_ROOT ?? '');
if (!output || process.env.AI_STUDIO_PERF_ISOLATED !== 'YES' ||
    dataRoot !== resolve(process.env.TEMP, 'ai-studio-phase12-native')) throw Error('Explicit owned isolation required');
const inventory = JSON.parse(execFileSync('python', ['-c', `
import sqlite3,json,sys
from pathlib import Path
c=sqlite3.connect(Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True)
tables=['tasks','assets','prompt_entries']
counts={t:c.execute('SELECT count(*) FROM '+t+" WHERE project_id='prj_default'").fetchone()[0] for t in tables}
counts['foreignKeyViolations']=len(c.execute('PRAGMA foreign_key_check').fetchall())
print(json.dumps(counts));c.close()
`, join(dataRoot, 'app.db')], { encoding: 'utf8', windowsHide: true }));
if (inventory.tasks < 50 || inventory.assets < 100 || inventory.prompt_entries < 50 || inventory.foreignKeyViolations) {
  throw Error('Representative isolated dataset gate failed');
}
const session = await connectNative();
const result = {
  phase: 12, type: 'INSTRUMENTED_DEEP_NATIVE',
  sourceHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  sourceWorktreeDirty: Boolean(execFileSync('git', ['status', '--porcelain', '--', 'src', 'src-tauri/src'], { encoding: 'utf8' }).trim()),
  frontendBundleIndexSha256: createHash('sha256').update(readFileSync('dist/index.html')).digest('hex'),
  backendSourceHead: process.env.AI_STUDIO_PERF_BACKEND_SOURCE_HEAD ?? 'NOT_RECORDED',
  dataset: { identity: 'phase12-large-v1', inventory, rootKind: 'owned isolated synthetic expansion' },
  buildMode: 'DEBUG', frontendMode: 'PRODUCTION_BUNDLE', scenarios: {}, resources: {},
  limitations: ['Acceptance instrumentation adds overhead; no comparison with uninstrumented latency',
    'Root commits are real React callbacks, not per-component render durations',
    'Production minification removes stable component names; owner names are not relied on',
    'Event/interval/blob registrations are not heap reachability or arbitrary JS subscription counts',
    'SQL count remains NOT_MEASURED in this Native helper'],
};
const readProfile = () => session.evaluate('window.__phase12Acceptance.snapshot()');
async function prepare(label, selector) { await session.click(label); await session.settle(selector); }
async function selectFirst(selector) {
  await session.evaluate(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});if(!e||e.disabled)throw Error('Expected fixture selection missing');e.click()})()`);
}
async function project(id) {
  await session.evaluate(`(()=>{const s=document.querySelector('.v3-project-picker select');
    if(![...s.options].some(o=>o.value===${JSON.stringify(id)}))throw Error('Fixture project missing');
    s.value=${JSON.stringify(id)};s.dispatchEvent(new Event('change',{bubbles:true}))})()`);
  await session.settle('.v3-shell');
  await prepare('概览', '.v3-overview');
}
const snapshot = () => writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
try {
  await session.settle('.v3-shell');
  if (!await session.evaluate("!!document.querySelector('.v3-project-picker select')")) {
    throw Error('Current fixture needs a project-scoped route before profiling');
  }
  await project('prj_default'); await prepare('概览', '.v3-overview');
  const script = readFileSync('scripts/testing/native-browser-profiler.js', 'utf8');
  await session.cdp('Page.enable');
  await session.cdp('Runtime.enable');
  await session.cdp('Page.addScriptToEvaluateOnNewDocument', { source: script });
  // Profiling preparation only; this reload is NEVER reported as startup/resume.
  await session.cdp('Page.reload');
  await session.settle('.v3-overview', 'typeof window.__phase12Acceptance!=="undefined"');
  const initial = await readProfile();
  if (!initial?.rendererCount || !initial.rootCommits) throw Error('Actual React hook not attached');
  const scenarios = [
    ['project-open', async () => {
      await project('prj_19ce69c5-f438-44a3-a0fb-92e41c569f98');
      await prepare('AI Studio · 项目', '.project-table');
    }, async () => {
      await session.evaluate(`(()=>{const row=[...document.querySelectorAll('.project-table-row')].find(r=>/默认项目|Default Project/.test(r.querySelector('strong')?.textContent??''));
        const b=[...row?.querySelectorAll('button')??[]].find(b=>b.textContent.trim()==='打开'&&!b.disabled);
        if(!b)throw Error('Fixture project Open button missing');b.click()})()`);
    }, '.v3-overview'],
    ['create-load', () => prepare('概览', '.v3-overview'), () => session.click('创作'), '.create-page'],
    ['create-shot-change', () => prepare('创作', '.create-page'), async () => {
      await session.evaluate(`(()=>{const s=document.querySelector('select[aria-label="镜头"]');const o=[...s.options].find(o=>o.value);
        if(!o)throw Error('Fixture shot missing');s.value=o.value;s.dispatchEvent(new Event('change',{bubbles:true}))})()`);
    }, '.create-content'],
    ['runs-load-large', () => prepare('概览', '.v3-overview'), () => session.click('运行'), '.runs-page'],
    ['runs-detail-large', () => prepare('运行', '.runs-page'), () => selectFirst('.run-list button'), '.run-detail[aria-label="运行详情"]'],
    ['library-load-large', () => prepare('运行', '.runs-page'), () => session.click('素材库'), '.library-page'],
    ['library-media-detail', () => prepare('素材库', '.library-page'), () => selectFirst('.library-card'), '.library-detail[aria-label="资源详情"]'],
    ['library-prompt-detail', async () => {
      await prepare('素材库', '.library-page'); await prepare('提示词', '.library-page');
    }, () => selectFirst('.library-card'), '.library-prompt-text'],
    ['library-prompt-edit', async () => {
      await prepare('素材库', '.library-page'); await prepare('提示词', '.library-page');
      await selectFirst('.library-card'); await session.settle('.library-prompt-text');
      await session.click('编辑'); await session.settle('.library-editor');
      await session.evaluate(`(()=>{const t=document.querySelector('textarea[aria-label="新版本正文"]');
        Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(t,'Isolated Phase12 version '+Date.now());
        t.dispatchEvent(new Event('input',{bubbles:true}))})()`);
    }, () => session.click('保存新版本'), '.library-prompt-text'],
  ];
  for (const [name, preparation, action, selector] of scenarios) {
    result.scenarios[name] = await measureScenario({
      prepare: async () => { await preparation(); session.reset(); await session.evaluate('window.__phase12Acceptance.resetCommits()'); },
      action,
      ready: async () => {
        await session.settle(selector, name === 'library-prompt-edit'
          ? "!document.querySelector('.library-editor')&&!window.__phase12Acceptance.snapshot().resources.timeoutPeriodsMs.includes(150)"
          : 'true');
        return { ipc: session.snapshot(), profiling: await readProfile() };
      },
    });
    snapshot();
    console.log(JSON.stringify({ scenario: name, median: result.scenarios[name].median,
      ipc: result.scenarios[name].raw.map(s => s.ipc.realIpc),
      commits: result.scenarios[name].raw.map(s => s.profiling.rootCommits) }));
  }
  // Resource transitions are separate from latency samples and mutation timings.
  await prepare('概览', '.v3-overview');
  result.resources.before = (await readProfile()).resources;
  result.resources.transitions = [];
  for (let index = 0; index < 20; index++) {
    await prepare('运行', '.runs-page');
    const mounted = (await readProfile()).resources;
    await prepare('概览', '.v3-overview');
    result.resources.transitions.push({ index: index + 1, mounted, afterLeave: (await readProfile()).resources });
    snapshot();
  }
  result.resources.after = (await readProfile()).resources;
  result.resources.subscriptionGate = 'NOT_MEASURED; private notification Set not intercepted';
} catch (error) { result.error = error.message; throw error; }
finally { snapshot(); session.close(); }
