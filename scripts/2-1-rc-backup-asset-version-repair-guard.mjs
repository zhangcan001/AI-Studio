// P1 identity successor: validate live bytes before projecting immutable release history.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const RC_REPAIR_PARENT = 'ed2f0c72e3f77c93b51f60e6af5475e51abb7e0b';
const existing = ["src-tauri/src/domain/asset.rs", "src-tauri/src/application/project_backup_service.rs", "src-tauri/src/infrastructure/database/repositories/asset.rs", "src-tauri/src/infrastructure/database/repositories/provenance_lineage.rs", "src-tauri/tests/product_library_contract.rs", "src-tauri/tests/dev051_consistency_assets.rs", "scripts/ai-studio-2-1-closeout-boundary-guard.mjs", "scripts/phase14-release-guard.mjs", "src/app/AIStudio21CloseoutBoundary.test.ts"].sort();
const added = ["scripts/2-1-rc-backup-asset-version-repair-guard.mjs", "src/app/RCBackupAssetVersionRepairBoundary.test.ts"].sort();
const normalize = s => s.replaceAll('\r\n','\n');
const hash = s => createHash('sha256').update(normalize(s)).digest('hex');
const files = (root,dir,pattern) => readdirSync(join(root,dir),{withFileTypes:true}).flatMap(e=>{
  const p=`${dir}/${e.name}`; return e.isDirectory()?files(root,p,pattern):pattern.test(p)?[p]:[];
}).sort();
const aggregate = (paths,read) => hash(paths.map(p=>`${p}\n${read(p)}`).join('\n'));
const immutable = new Map();
function parentBlobs(root) {
  if(immutable.has(root))return immutable.get(root);
  const paths=execFileSync('git',['ls-tree','-r','--name-only',RC_REPAIR_PARENT,'src','src-tauri/src','src-tauri/tests','scripts','src-tauri/migrations','docs/architecture','package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','.github/workflows/ci.yml',...existing],{cwd:root,encoding:'utf8'}).trim().split(/\r?\n/).sort();
  const bytes=execFileSync('git',['cat-file','--batch'],{cwd:root,input:paths.map(p=>`${RC_REPAIR_PARENT}:${p}\n`).join(''),maxBuffer:64*1024*1024});
  const blobs=new Map();let offset=0;
  for(const p of paths){const end=bytes.indexOf(10,offset),m=/^[a-f0-9]+ blob (\d+)$/.exec(bytes.subarray(offset,end).toString());if(!m)throw Error('Closeout parent blob missing');const start=end+1,length=Number(m[1]);if(bytes[start+length]!==10)throw Error('Closeout parent blob boundary');blobs.set(p,normalize(bytes.subarray(start,start+length).toString('utf8')));offset=start+length+1;}
  if(offset!==bytes.length)throw Error('Closeout parent trailing bytes');immutable.set(root,blobs);return blobs;
}
export function rcRepairParentReader(root,override) {
  const disk=p=>normalize(readFileSync(join(root,p),'utf8'));
  const proof=override??JSON.parse(disk('docs/architecture/2-1-rc-backup-asset-version-repair.json'));
  const violations=[],fail=s=>violations.push(`rc-repair-${s}`);
  if(proof.schemaVersion!==1||proof.checkpoint!=='AI_STUDIO_2_1_RC_BACKUP_ASSET_VERSION_IDENTITY_REPAIR'||proof.parentHead!==RC_REPAIR_PARENT)fail('header');
  if(JSON.stringify(Object.keys(proof.paths??{}).sort())!==JSON.stringify([...existing,...added].sort()))fail('scope');
  for(const flag of ['legacyAsvNewWrites','schemaChanged','backupFormatChanged','queueAuthorityChanged','taskStateMachineChanged','workflowCompilerChanged','comfySubmissionChanged','bindingOccChanged','remoteTelemetry'])if(proof.invariants?.[flag]!==false)fail(`invariant:${flag}`);
  if(proof.invariants?.assetVersionCanonicalPrefix!=='av_'||proof.invariants?.legacyAsvReadOnly!==true)fail('identity-contract');
  const rejected=()=>({violations,addedPaths:[],afterHashes:{},read:disk});
  if(violations.length)return rejected();
  const blobs=parentBlobs(root),before=p=>blobs.get(p);
  for(const p of existing)if(proof.paths[p].beforeHash!==hash(before(p))||proof.paths[p].afterHash!==hash(disk(p)))fail(`path:${p}`);
  for(const p of added)if(blobs.has(p)||proof.paths[p].beforeHash!==null||proof.paths[p].afterHash!==hash(disk(p)))fail(`addition:${p}`);
  for(const [name,dir,pattern] of [['backend','src-tauri/src',/\.rs$/],['tests','src-tauri/tests',/\.rs$/],['frontend','src',/\.tsx?$/],['styles','src',/\.css$/],['scripts','scripts',/\.(?:mjs|ts)$/]]){
    const base=[...blobs.keys()].filter(p=>p.startsWith(`${dir}/`)&&pattern.test(p)),current=files(root,dir,pattern),expected=[...base,...added.filter(p=>p.startsWith(`${dir}/`)&&pattern.test(p))].sort(),untouched=base.filter(p=>!existing.includes(p)),r=proof[name];
    if(JSON.stringify(current)!==JSON.stringify(expected)||r?.beforeFiles!==base.length||r?.afterFiles!==current.length)fail(`${name}-files`);
    if(r?.beforeAggregateHash!==aggregate(base,before)||r?.afterAggregateHash!==aggregate(current,disk)||r?.untouchedAggregateHash!==aggregate(untouched,before)||r?.untouchedAggregateHash!==aggregate(untouched,disk))fail(`${name}-aggregate`);
  }
  for(const p of [...blobs.keys()].filter(p=>p.startsWith('docs/architecture/')||p.startsWith('src-tauri/migrations/')))if(disk(p)!==before(p))fail(`frozen:${p}`);
  if(JSON.stringify(files(root,'src-tauri/migrations',/\.sql$/))!==JSON.stringify([...blobs.keys()].filter(p=>p.startsWith('src-tauri/migrations/')&&p.endsWith('.sql'))))fail('migration-file-set');
  for(const p of ['package.json','pnpm-lock.yaml','src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','.github/workflows/ci.yml'])if(disk(p)!==before(p))fail(`frozen-config:${p}`);
  const asset=disk('src-tauri/src/domain/asset.rs'),restore=disk('src-tauri/src/application/project_backup_service.rs');
  const publicParser=asset.slice(asset.indexOf('impl AssetVersionId {')).split('pub(crate) fn parse_persisted')[0];
  if(publicParser.includes('starts_with("asv_")'))fail('public-parser-legacy');
  if(!restore.includes('AssetVersionId::new().as_str().to_owned()')||restore.split('#[cfg(test)]')[0].includes('format!("asv_'))fail('restore-allocator');
  if(!restore.includes('const BACKUP_VERSION: u32 = 20;'))fail('backup-format');
  if(violations.length)return rejected();
  return {violations,addedPaths:added,afterHashes:Object.fromEntries(existing.map(p=>[p,proof.paths[p].afterHash])),backendAggregateSha256:proof.backend.afterAggregateHash,read:p=>existing.includes(p)?before(p):disk(p)};
}
