// Authorize Phase 6D scale/integrity/E2E-mock rollup before replaying Phase6C or older proofs.
// Historical manifests, Runtime Package bytes, migrations and BACKUP_VERSION stay immutable.
import { cachedBoundary, immutableGit } from './boundary-validation-cache.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { uiPhase7AParentReader, phase7AFixtureFiles } from './ui-phase7a-source-ready-successor-guard.mjs';

export const PHASE6D_PARENT = 'acbc458671e4f9b44f548f4c9c97a035168a7a5e';
export const PHASE6D_MANIFEST = 'docs/architecture/ui-phase6d-scale-integrity-successor.json';
export const PHASE6D_EXISTING = [
  'scripts/ui-phase6c-runs-queue-unify-successor-guard.mjs',
  'src-tauri/tests/support/library_scale_baseline.rs',
].sort();
export const PHASE6D_ADDED = [
  'scripts/ui-phase6d-scale-integrity-successor-guard.mjs',
  'src/app/UiPhase6DScaleIntegrityBoundary.test.ts',
  'src/features/Phase6DProductPath.e2e.test.tsx',
].sort();
export const PHASE6D_FILES = [...PHASE6D_EXISTING, ...PHASE6D_ADDED, PHASE6D_MANIFEST];
export const PHASE6D_INVARIANTS = {
  schemaChanged: false, backupFormatChanged: false, backendChanged: false,
  queueAuthorityChanged: false, storeAuthorityChanged: false, runtimePackagesChanged: false,
  historicalProofsRewritten: false, nestedSuccessStagingOnCommitOnly: true,
  libraryScaleIsolatedDb: true, observedTimingsNoFakePercent: true,
  e2eMockPathNoRealGeneration: true, mediaIntegrityNoDbCorruption: true,
  historicalReadableBackupRestoreVerified: true, phase6aThrough6cSemanticsPreserved: true,
  migrationUnchanged: true, backupVersionUnchanged: true,
  phase5fThrough6cPreserved: true, rawRuntimeBytesPreserved: true, sourceOnly: true,
  nativeGpuExecuted: false, phase6Started: true, phase6dCompleted: true,
};
export const PHASE6D_DATABASE = { migrationBefore: 44, migrationAfter: 44, newTables: 0, backupBefore: 21, backupAfter: 21 };
export const PHASE6D_GROUPS = [
  ['backend','src-tauri/src',/\.rs$/], ['tests','src-tauri/tests',/\.rs$/],
  ['frontend','src',/\.tsx?$/], ['styles','src',/\.css$/],
  ['scripts','scripts',/\.(mjs|ts)$/], ['migrations','src-tauri/migrations',/\.sql$/],
  ['architecture','docs/architecture',/\.json$/], ['packages','src-tauri/runtime_packages',/\.(yaml|json)$/],
  ['domainDocs','docs/architecture',/\.md$/],
];
const directories = [...new Set([...PHASE6D_GROUPS.map(([, dir]) => dir), 'src-tauri/examples','docs','.github'])];
const configs = ['CONTEXT.md','README.md','package.json','pnpm-lock.yaml','vite.config.ts',
  'src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src-tauri/build.rs',
  '.github/workflows/ci.yml','docs/AI_STUDIO_2_1_CLOSEOUT.md','docs/RELEASE_NOTES_v2.1.0-personal.md'];
const normalize = text => text.replaceAll('\r\n','\n');
export const phase6DHash = text => createHash('sha256').update(normalize(text)).digest('hex');
const rawHash = bytes => createHash('sha256').update(bytes).digest('hex');
export const phase6DAggregate = (paths, read) => phase6DHash(paths.map(p => `${p}\n${read(p)}`).join('\n'));
const parents = new Map();
export function phase6DParentFacts(root) {
  if (parents.has(root)) return parents.get(root);
  const paths = immutableGit('git',['ls-tree','-r','--name-only',PHASE6D_PARENT],{cwd:root,encoding:'utf8'})
    .trim().split(/\r?\n/).filter(p => configs.includes(p) || directories.some(dir => p.startsWith(dir+'/'))) .sort();
  const bytes = immutableGit('git',['cat-file','--batch'],{
    cwd:root,input:paths.map(p => `${PHASE6D_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024,
  });
  const blobs = new Map(); let offset = 0;
  for (const p of paths) {
    const end = bytes.indexOf(10,offset), match = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset,end).toString());
    if (!match) throw Error('Phase6D immutable parent missing');
    const start = end+1, length = Number(match[2]);
    if (bytes[start+length] !== 10) throw Error('Phase6D immutable blob boundary');
    blobs.set(p,bytes.subarray(start,start+length));offset=start+length+1;
  }
  if (offset !== bytes.length) throw Error('Phase6D immutable trailing bytes');
  const facts={paths,read:p=>normalize(blobs.get(p).toString('utf8')),raw:p=>blobs.get(p)};
  parents.set(root,facts);return facts;
}
function files(root,dir) {
  return readdirSync(join(root,dir),{withFileTypes:true}).flatMap(entry=>{
    const p=`${dir}/${entry.name}`;
    if(entry.isSymbolicLink()) throw Error('Phase6D symbolic source');
    return entry.isDirectory()?files(root,p):[p];
  }).sort();
}
export const phase6DFixtureFiles = root => [...new Set([...phase6DParentFacts(root).paths,...PHASE6D_FILES,...phase7AFixtureFiles(root)])];
function validate(root,override) {
  const successor=uiPhase7AParentReader(root);
  const violations=[...successor.violations],fail=name=>violations.push('ui-phase6d-scale-integrity-'+name);
  const live=new Map(),read=p=>{
    if(!live.has(p))live.set(p,normalize(successor.read(p)));
    return live.get(p);
  };
  const denied=()=>({violations,addedPaths:[],afterHashes:{},read:p=>normalize(readFileSync(join(root,p),'utf8'))});
  let proof,parent;
  try {proof=override??JSON.parse(read(PHASE6D_MANIFEST));parent=phase6DParentFacts(root);}catch{fail('missing-evidence');return denied();}
  const scope=[...PHASE6D_EXISTING,...PHASE6D_ADDED].sort();
  const proofKeys=['schemaVersion','checkpoint','parentHead','planningParentHead','phaseCommitsBeforeCheckpoint',
    'invariants','database','paths',...PHASE6D_GROUPS.map(([name])=>name)].sort();
  if(JSON.stringify(Object.keys(proof).sort())!==JSON.stringify(proofKeys)
    ||read(PHASE6D_MANIFEST)!==JSON.stringify(JSON.parse(read(PHASE6D_MANIFEST)),null,2)+'\n')fail('proof-bytes');
  if(proof.schemaVersion!==1||proof.checkpoint!=='UI_PHASE6D_SCALE_INTEGRITY_SUCCESSOR'
    ||proof.parentHead!==PHASE6D_PARENT||proof.planningParentHead!==PHASE6D_PARENT
    ||JSON.stringify(proof.phaseCommitsBeforeCheckpoint)!=='[]')fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify(scope))fail('scope');
  if(JSON.stringify(proof.invariants)!==JSON.stringify(PHASE6D_INVARIANTS)
    ||JSON.stringify(proof.database)!==JSON.stringify(PHASE6D_DATABASE))fail('contract');
  if(violations.length>successor.violations.length)return denied();
  try {
    for(const p of PHASE6D_EXISTING){
      if(!parent.paths.includes(p)||proof.paths[p]?.beforeHash!==phase6DHash(parent.read(p))
        ||proof.paths[p]?.afterHash!==phase6DHash(read(p)))fail('path:'+p);
    }
    for(const p of PHASE6D_ADDED){
      if(parent.paths.includes(p)||proof.paths[p]?.beforeHash!==null
        ||proof.paths[p]?.afterHash!==phase6DHash(read(p)))fail('addition:'+p);
    }
    for(const dir of directories){
      const expected=[...parent.paths.filter(p=>p.startsWith(dir+'/')),
        ...PHASE6D_ADDED.filter(p=>p.startsWith(dir+'/')),
        ...[PHASE6D_MANIFEST].filter(p=>p.startsWith(dir+'/')),
        ...successor.addedPaths.filter(p=>p.startsWith(dir+'/'))].sort();
      if(JSON.stringify(files(root,dir))!==JSON.stringify(expected))fail('inventory:'+dir);
    }
    for(const p of parent.paths.filter(p=>!PHASE6D_EXISTING.includes(p))){
      if(read(p)!==parent.read(p))fail('untouched:'+p);
      if(p.startsWith('src-tauri/migrations/')
        &&rawHash(readFileSync(join(root,p)))!==rawHash(parent.raw(p)))fail('migration-bytes:'+p);
    }
    for(const [name,dir,pattern] of PHASE6D_GROUPS){
      const base=parent.paths.filter(p=>p.startsWith(dir+'/')&&pattern.test(p));
      const current=[...base,...PHASE6D_ADDED.filter(p=>p.startsWith(dir+'/')&&pattern.test(p))].sort();
      const untouched=base.filter(p=>!PHASE6D_EXISTING.includes(p)),record=proof[name];
      if(record?.beforeFiles!==base.length||record?.afterFiles!==current.length
        ||record?.beforeAggregateHash!==phase6DAggregate(base,parent.read)
        ||record?.untouchedAggregateHash!==phase6DAggregate(untouched,parent.read)
        ||record?.afterAggregateHash!==phase6DAggregate(current,read))fail('aggregate:'+name);
    }
    const baselines=JSON.parse(parent.read('docs/architecture/minimax-video-phase2.json')).runtimeByteBaselines;
    const profile=process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE??'git';
    const packages=parent.paths.filter(p=>p.startsWith('src-tauri/runtime_packages/'));
    const rawAggregate=readBytes=>rawHash(JSON.stringify(packages.map(p=>[p,rawHash(readBytes(p))])));
    if(!Object.hasOwn(baselines,profile)||rawAggregate(parent.raw)!==baselines.git
      ||rawAggregate(p=>readFileSync(join(root,p)))!==baselines[profile])fail('runtime-package-bytes');
    const scale=read('src-tauri/tests/support/library_scale_baseline.rs');
    if(!scale.includes('phase6d_library_scale_paging_stability_and_source_filter')
      ||!scale.includes('AI_STUDIO_PHASE6D_SCALE_OUTPUT')
      ||!scale.includes('performanceClaim'))fail('scale-correctness');
    const e2e=read('src/features/Phase6DProductPath.e2e.test.tsx');
    if(!e2e.includes('phase6d mock path')||!e2e.includes('确认将生成视频仅作为视频参考')
      ||!e2e.includes('审核通过')||!e2e.includes('inputsSave')
      ||!e2e.includes('expect(api.generate).not.toHaveBeenCalled()')
      ||e2e.includes('workflow_dispatch'))fail('e2e-mock-path');
    for(const frozen of [
      'docs/architecture/ui-phase6c-runs-queue-unify-successor.json',
      'docs/architecture/ui-phase6b-library-shot-bind-successor.json',
      'docs/architecture/ui-phase6a-library-source-successor.json',
      'docs/architecture/ui-phase5j-ci-per-file-worker-successor.json',
    ]) if(read(frozen)!==parent.read(frozen))fail('frozen-proof:'+frozen);
  }catch{fail('missing-evidence');}
  if(violations.length)return denied();
  return {violations,addedPaths:[...PHASE6D_ADDED,PHASE6D_MANIFEST,...successor.addedPaths],
    afterHashes:{...Object.fromEntries(scope.map(p=>[p,proof.paths[p].afterHash])),...successor.afterHashes},
    read:p=>PHASE6D_EXISTING.includes(p)?parent.read(p):read(p)};
}
export function uiPhase6DParentReader(root,override) {
  return cachedBoundary(root,'uiPhase6DParentReader',override,()=>validate(root,override),PHASE6D_MANIFEST);
}
