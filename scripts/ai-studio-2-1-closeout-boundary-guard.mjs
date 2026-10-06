// Release-only successor. No historical manifest, runtime authority or CI exemption.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

import { rcRepairParentReader } from './2-1-rc-backup-asset-version-repair-guard.mjs';

export const CLOSEOUT_PARENT = '76a261607021d2ad10baef740d0afe9aebb6fc6f';
const existing = ['package.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','scripts/testing/release-config.test.ts','scripts/m4-readonly-media-integrity-boundary-guard.mjs','src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs'].sort();
const added = ['scripts/ai-studio-2-1-closeout-boundary-guard.mjs','src/app/AIStudio21CloseoutBoundary.test.ts'].sort();
const normalize = s => s.replaceAll('\r\n','\n');
const hash = s => createHash('sha256').update(normalize(s)).digest('hex');
const files = (root,dir,pattern) => readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e=>{
  const p=`${dir}/${e.name}`; return e.isDirectory()?files(root,p,pattern):pattern.test(p)?[p]:[];
}).sort();
const aggregate = (paths,read) => hash(paths.map(p=>`${p}\n${read(p)}`).join('\n'));
const immutable = new Map();
function parentBlobs(root) {
  if(immutable.has(root))return immutable.get(root);
  const paths=execFileSync('git',['ls-tree','-r','--name-only',CLOSEOUT_PARENT,'src','src-tauri/src','scripts','src-tauri/migrations','docs/architecture',...existing],{cwd:root,encoding:'utf8'}).trim().split(/\r?\n/).sort();
  const bytes=execFileSync('git',['cat-file','--batch'],{cwd:root,input:paths.map(p=>`${CLOSEOUT_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024});
  const blobs=new Map();let offset=0;
  for(const p of paths){const end=bytes.indexOf(10,offset),m=/^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset,end).toString());if(!m)throw Error('Closeout parent blob missing');const start=end+1,length=Number(m[1]);if(bytes[start+length]!==10)throw Error('Closeout parent blob boundary');blobs.set(p,normalize(bytes.subarray(start,start+length).toString('utf8')));offset=start+length+1;}
  if(offset!==bytes.length)throw Error('Closeout parent trailing bytes');immutable.set(root,blobs);return blobs;
}
export function closeoutParentReader(root,override) {
  const repair=rcRepairParentReader(root);
  const disk=p=>normalize(repair.read(p));
  const proof=override??JSON.parse(disk('docs/architecture/ai-studio-2-1-closeout.json'));
  const violations=[...repair.violations],fail=s=>violations.push(`closeout-${s}`);
  if(proof.schemaVersion!==1||proof.checkpoint!=='AI_STUDIO_2_1_CLOSEOUT'||proof.parentHead!==CLOSEOUT_PARENT)fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...existing,...added].sort()))fail('scope');
  for(const flag of ['businessRuntimeChanged','schemaChanged','backupFormatChanged','queueAuthorityChanged','taskStateMachineChanged','workflowCompilerChanged','comfySubmissionChanged','bindingOccChanged','remoteTelemetry'])if(proof.invariants?.[flag]!==false)fail(`invariant:${flag}`);
  const rejected=()=>({violations,addedPaths:[],afterHashes:{},read:disk});
  if(violations.length)return rejected();
  const blobs=parentBlobs(root),before=p=>blobs.get(p);
  for(const p of existing)if(proof.paths[p].beforeHash!==hash(before(p))||proof.paths[p].afterHash!==hash(disk(p)))fail(`path:${p}`);
  for(const p of added)if(blobs.has(p)||proof.paths[p].beforeHash!==null||proof.paths[p].afterHash!==hash(disk(p)))fail(`addition:${p}`);
  for(const [name,dir,pattern] of [['backend','src-tauri/src',/\.rs$/],['frontend','src',/\.tsx?$/],['styles','src',/\.css$/],['scripts','scripts',/\.(?:mjs|ts)$/]]){
    const base=[...blobs.keys()].filter(p=>p.startsWith(`${dir}/`)&&pattern.test(p)),current=files(root,dir,pattern).filter(p=>!repair.addedPaths.includes(p)),expected=[...base,...added.filter(p=>p.startsWith(`${dir}/`)&&pattern.test(p))].sort(),untouched=base.filter(p=>!existing.includes(p)),r=proof[name];
    if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
    if(r?.beforeAggregateHash!==aggregate(base,before)||r?.afterAggregateHash!==aggregate(current,disk)||r?.untouchedAggregateHash!==aggregate(untouched,before)||r?.untouchedAggregateHash!==aggregate(untouched,disk))fail(`${name}-aggregate`);
  }
  const pkg=JSON.parse(disk('package.json')),config=JSON.parse(disk('src-tauri/tauri.conf.json'));
  if(pkg.version!=='2.1.0-personal'||config.version!==pkg.version||config.bundle.windows.wix.version!=='2.1.0'||!disk('src-tauri/Cargo.toml').includes('version = "2.1.0-personal"'))fail('versions');
  if(disk('package.json')!==before('package.json').replace('"version": "2.0.0-personal"','"version": "2.1.0-personal"')||disk('src-tauri/Cargo.toml')!==before('src-tauri/Cargo.toml').replace('version = "2.0.0-personal"','version = "2.1.0-personal"'))fail('version-only');
  if(disk('src-tauri/Cargo.lock')!==before('src-tauri/Cargo.lock').replace('name = "ai-studio"\nversion = "2.0.0-personal"','name = "ai-studio"\nversion = "2.1.0-personal"'))fail('lock-only');
  if(disk('src-tauri/tauri.conf.json')!==before('src-tauri/tauri.conf.json').replace('"version": "2.0.0-personal"','"version": "2.1.0-personal"').replace('"version": "2.0.0"','"version": "2.1.0"'))fail('bundle-only');
  for(const p of [...blobs.keys()].filter(p=>p.startsWith('docs/architecture/')||p.startsWith('src-tauri/migrations/')))if(disk(p)!==before(p))fail(`frozen:${p}`);
  if(JSON.stringify(files(root,'src-tauri/migrations',/\.sql$/))!==JSON.stringify([...blobs.keys()].filter(p=>p.startsWith('src-tauri/migrations/')&&p.endsWith('.sql'))))fail('migration-file-set');
  if(disk('scripts/testing/release-config.test.ts')!==before('scripts/testing/release-config.test.ts').replace("'2.0.0-personal'","'2.1.0-personal'"))fail('release-test-semantics');
  const rustReleaseTest='src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs';
  if(disk(rustReleaseTest)!==before(rustReleaseTest).replaceAll('2.0.0-personal','2.1.0-personal'))fail('rust-release-test-semantics');
  if(!disk('README.md').includes('NOT YET PUBLISHED')||!disk('README.md').includes('v2.0.0-personal-r4')||!disk('docs/AI_STUDIO_2_1_ROADMAP.md').includes('PRODUCT_WORK_COMPLETE=YES')||!disk('docs/RELEASE_NOTES_v2.1.0-personal.md').includes('This document does not announce a Git tag or GitHub Release.'))fail('publication-docs');
  if(violations.length)return rejected();
  return {violations,addedPaths:[...added,...repair.addedPaths],afterHashes:{...Object.fromEntries(existing.map(p=>[p,proof.paths[p].afterHash])),...repair.afterHashes},backendAggregateSha256:repair.backendAggregateSha256,read:p=>existing.includes(p)?before(p):disk(p)};
}
