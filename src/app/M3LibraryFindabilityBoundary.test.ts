// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only guard test.
import {readFileSync} from 'node:fs';
// @ts-expect-error Build-time guard.
import {m3LibraryParentReader} from '../../scripts/m3-library-findability-boundary-guard.mjs';
// @ts-expect-error Full successor chain.
import {m1ParentReader} from '../../scripts/m1-readiness-boundary-guard.mjs';
it('M3-1 validates parent/live paths and immutable baseline before historical projection, fails closed',()=>{
 const proof=JSON.parse(readFileSync('docs/architecture/m3-1-library-findability.json','utf8'));
 const successor=m3LibraryParentReader('.'),current=m1ParentReader('.');
 expect(successor.violations).toEqual([]);expect(current.violations).toEqual([]);
 expect(current.backendAggregateSha256).toBe(successor.backendAggregateSha256);
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths['src/product/libraryTypes.ts'].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
  (p:typeof proof)=>{p.frontend.afterFiles++;},
  ...Object.keys(proof.invariants).map(flag=>(p:typeof proof)=>{p.invariants[flag]=true;})
 ]){const invalid=structuredClone(proof);mutate(invalid);expect(m3LibraryParentReader('.',invalid).violations.length).toBeGreaterThan(0);}
},30000);
