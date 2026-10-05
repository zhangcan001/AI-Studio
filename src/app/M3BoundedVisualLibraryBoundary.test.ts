// @vitest-environment node
import {expect,it} from 'vitest';
// @ts-expect-error Node-only guard.
import {readFileSync} from 'node:fs';
// @ts-expect-error Build-time source validator.
import {m3VisualParentReader,visualCandidateViolations} from '../../scripts/m3-bounded-visual-library-boundary-guard.mjs';
// @ts-expect-error Complete historical successor chain.
import {m1ParentReader} from '../../scripts/m1-readiness-boundary-guard.mjs';
it('validates bounded visual Library successor before historical projection and rejects proof widening',()=>{
 const proof=JSON.parse(readFileSync('docs/architecture/m3-2-bounded-visual-library.json','utf8'));
 const current=m3VisualParentReader('.');expect(current.violations).toEqual([]);expect(m1ParentReader('.').violations).toEqual([]);
 expect(m1ParentReader('.').backendAggregateSha256).toBe(proof.backend.afterAggregateHash);
 for(const mutate of [
  (p:typeof proof)=>{p.parentHead='0'.repeat(40);},
  (p:typeof proof)=>{p.paths['src/features/library/LibraryController.ts'].afterHash='0'.repeat(64);},
  (p:typeof proof)=>{p.backend.untouchedAggregateHash='0'.repeat(64);},
  (p:typeof proof)=>{p.frontend.afterFiles++;},
  (p:typeof proof)=>{delete p.paths['src/product/client.ts'];},
  ...Object.keys(proof.invariants).map(flag=>(p:typeof proof)=>{p.invariants[flag]=true;})
 ]){const invalid=structuredClone(proof);mutate(invalid);expect(m3VisualParentReader('.',invalid).violations.length).toBeGreaterThan(0);}
},30000);
it('fixed performance targets fail closed rather than raising budgets after regressions',()=>{
 const baseline=JSON.parse(readFileSync('docs/architecture/m3-1-library-scale-baseline.json','utf8'));
 const candidate=JSON.parse(readFileSync('docs/architecture/m3-2-library-scale-candidate.json','utf8'));
 expect(visualCandidateViolations(baseline,candidate)).toEqual([]);
 for(const mutate of [
  (c:typeof candidate)=>{c.observations[0].productListCalls=210;},
  (c:typeof candidate)=>{c.observations[0].domItems=600;},
  (c:typeof candidate)=>{c.observations[0].p95Ms=99999;},
  (c:typeof candidate)=>{c.thumbnailObservation.offscreenRequestCount=1;},
  (c:typeof candidate)=>{c.thumbnailObservation.fullMediaReads=1;},
  (c:typeof candidate)=>{c.thumbnailObservation.objectUrlsRevoked=0;},
  (c:typeof candidate)=>{c.budgetAdjusted=true;},
 ]){const invalid=structuredClone(candidate);mutate(invalid);expect(visualCandidateViolations(baseline,invalid).length).toBeGreaterThan(0);}
});
