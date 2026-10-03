// Static discovery is not an executed test count. CI logs are the count/timing authority.
import { readFileSync, readdirSync, existsSync, writeFileSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

export function testInventory(root) {
  const rows = [], ignored = [], helpers = [];
  const backendMap = new Map(JSON.parse(readFileSync(`${root}/docs/architecture/phase8-backend-decomposition.json`, 'utf8')).rows.map(r => [r.path, r.bounded_context]));
  function walk(dir) {
    if (!existsSync(dir)) return;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (['node_modules', 'target', '.git', 'dist'].includes(entry.name)) continue;
      const path = `${dir}/${entry.name}`;
      if (entry.isDirectory()) walk(path);
      else if (/\.(?:tsx?|m?[cj]s|rs)$/.test(path)) inspect(path);
    }
  }
  function inspect(file) {
    const path = relative(root, file).replaceAll('\\', '/'), source = readFileSync(file, 'utf8');
    const rust = path.endsWith('.rs');
    let tests = [];
    if (rust) {
      // Attribute discovery only, not a Rust parser or expanded/cfg-aware count.
      tests = [...source.matchAll(/#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*((?:#\[[^\]]*\]\s*)*)(?:pub\s+)?(?:async\s+)?fn\s+(\w+)/g)].map(m => ({ name: m[2], ignored: /#\[ignore/.test(m[1]) }));
      for (const test of tests.filter(t => t.ignored)) ignored.push({ path, test: test.name, why: /live|real_workflow/.test(test.name) ? 'Explicit external resource / user-owned runtime or workflow required' : 'Inspect annotated resource requirement', category: 'MANUAL_RESOURCE', safe_to_ignore_in_offline_ci: true, substitute: 'Sanitized workflow, controlled adapters and isolated DB tests remain active' });
    } else if (/\.(test|spec)\./.test(path) || path.includes('/__tests__/')) {
      const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, file.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
      function visit(node) {
        if (ts.isCallExpression(node)) {
          const expression = node.expression.getText(ast);
          if (/^(?:it|test)(?:\.(?:only|skip|todo|each)(?:\([^]*\))?)?$/.test(expression) && node.arguments[0] && ts.isStringLiteralLike(node.arguments[0])) tests.push({ name: node.arguments[0].text, ignored: /\.(skip|todo)/.test(expression), parameterized: expression.includes('.each') });
        }
        ts.forEachChild(node, visit);
      }
      visit(ast);
    }
    if (/support\/|test[-_]?helpers|fixtures?\//i.test(path)) helpers.push({ path, kind: /db|database/i.test(source) ? 'DB_OR_DOMAIN_FIXTURE' : 'DOMAIN_HELPER', real_db: /initialize\(|SqlitePool/.test(source), real_fs: /tempdir|TempDir|mkdtemp/.test(source) });
    if (!tests.length) return;
    const owner = path.startsWith('src/features/') ? path.split('/')[2] : path.startsWith('src-tauri/src/') ? path.split('/')[2] : path.startsWith('src-tauri/tests') ? 'backend integration' : path.startsWith('src/app') ? 'application' : path.split('/')[1];
    const architecture = /Boundary|Retirement|Architecture|Consolidation|guard/i.test(path);
    const sourceAssertion = /readFileSync|read_to_string|include_str!/.test(source);
    const db = /initialize\(|SqlitePool|SqliteConnection|ProjectDatabase::new|Sqlite.*Repository/.test(source);
    const mock = /vi\.mock|mockResolvedValue|Mock[A-Z]|Fake[A-Z]|Stub[A-Z]/.test(source);
    const layer = architecture ? 'ARCHITECTURE' : /contract|ipc|serialization/i.test(path) ? 'CONTRACT' : path.startsWith('src-tauri/tests') ? 'INTEGRATION' : db ? 'REPOSITORY' : /src-tauri\/src\/application/.test(path) ? 'APPLICATION' : /src-tauri\/src\/domain/.test(path) ? 'DOMAIN' : /\.tsx$/.test(path) ? 'FEATURE' : 'UNIT';
    const sleeps = [...source.matchAll(/(?:thread::sleep|tokio::time::sleep|\bsleep|setTimeout)\s*\(/g)].length;
    const inferredContext = /workflow|recipe|binding/i.test(path) ? 'Workflow' : /production|generation|queue|run_|runtime|task|artifact/i.test(path) ? 'Production/Runs' : /asset|prompt|library|consistency|reference/i.test(path) ? 'Library/Assets' : /project|backup/i.test(path) ? 'Project' : /create|studio/i.test(path) ? 'Create/Studio' : 'System/Application';
    rows.push({ path, suite: path.replace(/\.(?:test|spec)?\.?[a-z]+$/, ''), owner, bounded_context: backendMap.get(path) ?? inferredContext,
      test_layer: layer, behavior_covered: tests.map(t => t.name), failure_mode: architecture ? 'Boundary drift / forbidden ownership' : 'Behavior, error, isolation or compatibility regression: see named cases',
      uses_mock: mock, uses_real_db: db, uses_real_fs: /tempdir|TempDir|mkdtemp|FileSystem|write_file/.test(source),
      uses_real_ipc: false, uses_real_process: /Command::new|execFileSync|spawn\(/.test(source), uses_native_app: false,
      runtime_class: db || /Command::new/.test(source) ? 'IO_BOUND_MEASURE_AT_CI' : 'CPU_OR_COMPONENT_MEASURE_AT_CI',
      flake_risk: sleeps || /127\.0\.0\.1:\d|localhost:\d/.test(source) ? 'REVIEW_SYNC_OR_RESOURCE' : 'NO_STATIC_CANDIDATE',
      duplication_group: /dev088-architecture-guard/.test(source) ? 'FULL_ARCHITECTURE_RUNNER' : null,
      fixture_strategy: db ? 'Per-case real isolated DB; audit TempDir lifetime' : mock ? 'Local typed adapter mocks; no real IPC claim' : 'Pure inputs / source fixtures',
      source_assertion_category: sourceAssertion ? architecture ? 'ARCHITECTURE_ASSERTION' : 'MIXED_REQUIRES_FRAGMENT_REVIEW' : null,
      source_assertion_sites: [...source.matchAll(/(?:expect\([^\n]*(?:source|code|create|app|pages)|assert!\([^\n]*contains)/g)].length,
      sleep_sites: sleeps, tests, decision: sleeps ? 'DEFER' : 'KEEP', replacement: null,
      risk: 'Static discovery is not exhaustive semantic proof; keep cross-layer coverage and authoritative CI' });
  }
  for (const dir of ['src', 'tests', 'src-tauri/src', 'src-tauri/tests']) walk(`${root}/${dir}`);
  return { rows, ignored, helpers, counts: { frontendFiles: rows.filter(r => r.path.startsWith('src/')).length, rustUnitDeclarations: rows.filter(r => r.path.startsWith('src-tauri/src')).reduce((n,r)=>n+r.tests.length,0), rustIntegrationDeclarations: rows.filter(r=>r.path.startsWith('src-tauri/tests')).reduce((n,r)=>n+r.tests.length,0), rustIgnoredDeclarations: ignored.length, sourceAssertionGroups: rows.filter(r=>r.source_assertion_category).length, snapshotGroups: rows.filter(r=>/toMatch(?:Inline)?Snapshot/.test(readFileSync(`${root}/${r.path}`, 'utf8'))).length, flakeCandidates: rows.filter(r=>r.flake_risk==='REVIEW_SYNC_OR_RESOURCE').length } };
}

export function ciMeasurements(ci, frontendLog, rustLog) {
  const clean = s => s.replace(/\u001b\[[0-9;]*m/g, '');
  frontendLog = clean(frontendLog); rustLog = clean(rustLog);
  let target = '', suites = [];
  for (const line of rustLog.split('\n')) {
    const running = line.match(/Running (?:unittests|tests\\)([^]*?) \(/);
    if (running) target = running[1].trim();
    const result = line.match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;[^]*finished in ([\d.]+)s/);
    if (result) suites.push({ target, passed: +result[1], failed: +result[2], ignored: +result[3], seconds: +result[4] });
  }
  const timings = ci.jobs.map(j => ({ name:j.name, seconds:(Date.parse(j.completedAt)-Date.parse(j.startedAt))/1000, steps:j.steps.filter(s=>s.startedAt&&s.completedAt).map(s=>({name:s.name,seconds:(Date.parse(s.completedAt)-Date.parse(s.startedAt))/1000,conclusion:s.conclusion})) }));
  return { run:ci.databaseId,head:ci.headSha,status:ci.status,conclusion:ci.conclusion,
    frontendTests:+(frontendLog.match(/Tests\s+(\d+) passed/)?.[1]??0),frontendFiles:+(frontendLog.match(/Test Files\s+(\d+) passed/)?.[1]??0),
    frontendSuiteSeconds:+(frontendLog.match(/Duration\s+([\d.]+)s/)?.[1]??0),
    frontendSuites:[...frontendLog.matchAll(/(?:✓|\u2713)\s+([^\n]*?\.(?:test|spec)\.[a-z]+)\s+\((\d+) tests?\)\s+(\d+)ms/g)].map(m=>({path:m[1].trim(),tests:+m[2],milliseconds:+m[3]})),
    rustSuites:suites,rustPassed:suites.reduce((n,s)=>n+s.passed,0),rustIgnored:suites.reduce((n,s)=>n+s.ignored,0),rustSuiteSeconds:suites.reduce((n,s)=>n+s.seconds,0),
    totalCiSeconds:(Math.max(...ci.jobs.map(j=>Date.parse(j.completedAt)))-Math.min(...ci.jobs.map(j=>Date.parse(j.startedAt))))/1000,jobs:timings };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root=process.cwd(),output=process.argv[2];
  const inventory=testInventory(root);
  if(output) writeFileSync(output,JSON.stringify(inventory,null,2)+'\n');
  else console.log(JSON.stringify(inventory.counts,null,2));
}
