// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only acceptance helper.
import {readFileSync} from 'node:fs';
// @ts-expect-error Build-time repair proof, not browser runtime.
import {rcRepairParentReader} from '../../scripts/2-1-rc-backup-asset-version-repair-guard.mjs';
it('validates canonical restore and persisted-only legacy compatibility before historical projection',()=>{
  const proof=JSON.parse(readFileSync('docs/architecture/2-1-rc-backup-asset-version-repair.json','utf8'));
  expect(rcRepairParentReader('.').violations).toEqual([]);
  for(const mutate of [
    (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
    (p:typeof proof)=>{delete p.paths['src-tauri/src/domain/asset.rs'];},
    (p:typeof proof)=>{p.paths['src-tauri/src/domain/asset.rs'].afterHash='0'.repeat(64);},
    (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
    (p:typeof proof)=>{p.tests.afterFiles++;},
    (p:typeof proof)=>{p.invariants.assetVersionCanonicalPrefix='asv_';},
    (p:typeof proof)=>{p.invariants.legacyAsvReadOnly=false;},
    ...Object.entries(proof.invariants).filter(([,v])=>v===false).map(([flag])=>(p:typeof proof)=>{p.invariants[flag]=true;})
  ]){const changed=structuredClone(proof);mutate(changed);const rejected=rcRepairParentReader('.',changed);expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);expect(rejected.afterHashes).toEqual({});}
},30000);
