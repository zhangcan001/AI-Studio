// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only acceptance helper.
import {readFileSync} from 'node:fs';
// @ts-expect-error Build-time release successor, not browser code.
import {closeoutParentReader} from '../../scripts/ai-studio-2-1-closeout-boundary-guard.mjs';
// @ts-expect-error Complete historical validation chain.
import {m1ParentReader} from '../../scripts/m1-readiness-boundary-guard.mjs';
it('validates version-only 2.1 release candidate before immutable M4/M3/M2/M1 projection and rejects drift',()=>{
  const proof=JSON.parse(readFileSync('docs/architecture/ai-studio-2-1-closeout.json','utf8'));
  expect(closeoutParentReader('.').violations).toEqual([]);
  expect(m1ParentReader('.').violations).toEqual([]);
  expect(m1ParentReader('.').backendAggregateSha256).toBe(proof.backend.afterAggregateHash);
  for(const mutate of [
    (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
    (p:typeof proof)=>{p.paths['src-tauri/Cargo.toml'].afterHash='0'.repeat(64);},
    (p:typeof proof)=>{delete p.paths['package.json'];},
    (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
    (p:typeof proof)=>{p.frontend.afterFiles++;},
    ...Object.keys(proof.invariants).map(flag=>(p:typeof proof)=>{p.invariants[flag]=true;})
  ]){const changed=structuredClone(proof);mutate(changed);const rejected=closeoutParentReader('.',changed);expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.addedPaths).toEqual([]);expect(rejected.afterHashes).toEqual({});}
},30000);
