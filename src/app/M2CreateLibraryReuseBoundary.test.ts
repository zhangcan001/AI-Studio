// @vitest-environment node
import { expect,it } from 'vitest';
// @ts-expect-error Node-only test helper.
import { readFileSync } from 'node:fs';
// @ts-expect-error Build-time guard.
import { m2ReuseParentReader } from '../../scripts/m2-create-library-reuse-boundary-guard.mjs';
// @ts-expect-error Full frozen chain.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';
it('validates exact M2-2 parent/live proof before immutable M2-1/M1 projection, failing closed',()=>{
 const proof=JSON.parse(readFileSync('docs/architecture/m2-2-create-library-reuse.json','utf8'));
 expect(m2ReuseParentReader('.').violations).toEqual([]); expect(m1ParentReader('.').violations).toEqual([]);
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths['src/product/types.ts'].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.paths['src/services/ipc.ts']={beforeHash:null,afterHash:'0'.repeat(64)};},
  (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
  (p:typeof proof)=>{p.frontend.afterFiles++;},
  (p:typeof proof)=>{p.scripts.afterAggregateHash='0'.repeat(64);},
  ...Object.keys(proof.invariants).map(flag=>(p:typeof proof)=>{p.invariants[flag]=true;})
 ]) { const invalid=structuredClone(proof);mutate(invalid); const gate=m2ReuseParentReader('.',invalid);
 expect(gate.violations.length).toBeGreaterThan(0); expect(gate.read('src/product/types.ts')).toContain('promptChoices: CreationPromptChoice[]'); }
},30000);
