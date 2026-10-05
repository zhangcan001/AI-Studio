// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only test helper.
import {readFileSync} from 'node:fs';
// @ts-expect-error Build-time read-only successor guard.
import {m4MediaParentReader} from '../../scripts/m4-readonly-media-integrity-boundary-guard.mjs';
// @ts-expect-error Full validated historical successor chain.
import {m1ParentReader} from '../../scripts/m1-readiness-boundary-guard.mjs';
it('validates readonly M4 inspection before projecting immutable M3/M2/M1 and rejects widening',()=>{
 const proof=JSON.parse(readFileSync('docs/architecture/m4-1-readonly-media-integrity.json','utf8'));
 const current=m4MediaParentReader('.'),chain=m1ParentReader('.');expect(current.violations).toEqual([]);expect(chain.violations).toEqual([]);expect(chain.backendAggregateSha256).toBe(current.backendAggregateSha256);
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths['src-tauri/src/application/asset_query_service/media_integrity.rs'].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
  (p:typeof proof)=>{p.frontend.afterFiles++;},
  (p:typeof proof)=>{delete p.paths['src/product/client.ts'];},
  ...Object.keys(proof.invariants).map(flag=>(p:typeof proof)=>{p.invariants[flag]=true;})
 ]){const invalid=structuredClone(proof);mutate(invalid);const rejected=m4MediaParentReader('.',invalid);expect(rejected.violations.length).toBeGreaterThan(0);expect(rejected.backendAggregateSha256).toBeUndefined();expect(rejected.addedCommandSignatures).toEqual([]);}
},30000);
