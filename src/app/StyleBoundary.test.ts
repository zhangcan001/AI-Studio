// @vitest-environment node
import { it, expect } from 'vitest';
// @ts-expect-error Node helpers are test-only.
import { readFileSync } from 'node:fs';
// @ts-expect-error Existing build-time guard module, not browser code.
import { styleBoundary, styleInventory, specificity, debtViolations, expandedCss, canonical, read, hash } from '../../scripts/style-boundary-guard.mjs';
// @ts-expect-error Build-time exact successor validator, not browser code.
import { phase13Boundary } from '../../scripts/phase13-observability-guard.mjs';
// @ts-expect-error Build-time current successor, not application code.
import { m1ParentReader } from '../../scripts/m1-readiness-boundary-guard.mjs';
// @ts-expect-error Validated M3-2 stylesheet successor; historical manifest stays immutable.
import { m3VisualParentReader } from '../../scripts/m3-bounded-visual-library-boundary-guard.mjs';
const m=JSON.parse(readFileSync('docs/architecture/phase9-style-cleanup.json','utf8'));
it('phase9_target1 accounts for all styles and classifies inline candidates',()=>{
 expect(m.BLOCKED_UNKNOWN).toBe(0);expect(m.rows.length).toBe(m.before.selectors);
 for(const r of m.rows){expect(['KEEP','DELETE_DEAD','MERGE_DUPLICATE','MOVE_TO_SHARED','MOVE_TO_FEATURE','MOVE_TO_PAGE','TOKENIZE','REDUCE_SPECIFICITY','KEEP_GLOBAL','KEEP_COMPAT','DEFER']).toContain(r.decision);expect(r.owner).toBeTruthy();expect(r.risk).toBeTruthy();}
 // Exact reviewed delta: one CSS line / three scoped thumbnail-pagination
 // selectors. All other inventory counts and debt budgets remain frozen.
 expect(m3VisualParentReader('.').violations).toEqual([]);
 const now=styleInventory('.');expect(now.metrics).toEqual({...m.after,lines:m.after.lines+1,selectors:m.after.selectors+3});expect(now.inline.every((r:{decision:string})=>['KEEP','DEFER'].includes(r.decision))).toBe(true);
},15000); // Full source/inline inventory; bounded static guard, not a browser test.
it('phase9_target2 prevents new important IDs deep and global selectors',()=>{
 expect(debtViolations({'new':{important:1,id:1,deep:1,global:1}},{})).toHaveLength(4);
 expect(debtViolations({'old':{important:2,id:0,deep:0,global:0}},{old:{important:1}})).toEqual(['important:old']);
 expect(specificity(':where(#host, .host) button')).toEqual([0,0,1]);expect(specificity('.a:has(#b, .c) > input')).toEqual([1,1,1]);
});
it('phase9_target3 guards ownership and preserves every production frontend source',()=>{
 expect(styleBoundary('.',m).violations).toEqual([]);
 const wrongOwner={...m,ownedFiles:{...m.ownedFiles,'src/features/create/CreatePage.css':['.workflow-']}};
 expect(styleBoundary('.',wrongOwner).violations.some((v:string)=>v.startsWith('ownership:'))).toBe(true);
 expect(m.ownedFiles['src/styles/modelVersionSelector.css']).toEqual(['.model-version-selector']);
 const shared=m.rows.find((r:{selector_or_scope:string})=>r.selector_or_scope==='.model-version-selector');expect(shared.callers).toContain('src/features/assets/AssetVideoBatchWorkspace.tsx');expect(shared.callers).toContain('src/features/workflow-lab/WorkflowLabSurface.tsx');
},30000); // Historical Git/source IO under CI contention, not a runtime performance threshold.
it('phase9_target4 preserves exact cascade ordering through eager imports',()=>{
 expect(canonical(expandedCss('src/app/App.css'))).toBe(m.proofs.extraction.canonicalBefore);
 expect(m.proofs.extraction.parts).toHaveLength(5);expect(read('src/app/App.tsx')).toContain('import "./App.css";');
});
it('phase9_target5 removes only the retired root without deleting active quality rules',()=>{
 expect(m.proofs.deadRoot).toHaveLength(58);
 for(const p of ['src/styles/studioQuality.css','src/styles/uiPolish.css','src/styles/studioTokens.css'])expect(read(p)).not.toMatch(/\.studio-shell\b/);
 expect(read('src/styles/studioQuality.css')).toContain('.startup-card');expect(read('src/styles/uiPolish.css')).toContain('.app-main-content');
 expect(read('src/styles/appCompatibility.css')).toContain('.provenance-timeline');expect(read('src/features/tasks/TaskHistoryDetail.tsx')).toContain('provenance-timeline');
});
it('phase9_target6 uses existing exact-value tokens without a parallel system',()=>{
 expect(m.proofs.tokens.newTokens).toBe(0);expect(m.proofs.tokens.replacements).toBe(47);
 expect(read('src/app/v3/AppShellV3.css')).toContain('var(--studio-space-4, 16px)');expect(read('src/styles/studioTokens.css')).toContain('--studio-space-4: 16px');
 expect(read('src/features/create/CreatePage.css')).toContain('--text-primary,#e5e7eb');
});
it('phase9_target7 lowers specificity and merges only the redundant media rule',()=>{
 expect(read('src/features/create/CreatePage.css')).toContain('.create-page :where(label)');expect(read('src/features/create/CreatePage.css')).not.toContain('row!important');
 expect(m.after.important).toBe(m.before.important-1);expect(m.proofs.duplicates.removed).toBe(1);expect(m.after.global).toBeLessThan(m.before.global);
});
it('phase9_target8 pins style successor while preserving all historical non-CSS freezes',()=>{
 const phase8=JSON.parse(read('docs/architecture/phase8-backend-decomposition.json'));
 const phase13=JSON.parse(read('docs/architecture/phase13-observability.json'));
 const checked=phase13Boundary('.',phase13,JSON.parse(read('docs/architecture/phase12-performance.json')),phase8.backendSourceSnapshot);
 expect(checked.violations).toEqual([]);
});
it('phase9_target8 preserves every historical non-CSS consumer through the validated successor',()=>{
 const phase8=JSON.parse(read('docs/architecture/phase8-backend-decomposition.json'));
 const phase10=JSON.parse(read('docs/architecture/phase10-frontend-architecture.json'));
 const phase13=JSON.parse(read('docs/architecture/phase13-observability.json'));
 const current=m1ParentReader('.');expect(current.violations).toEqual([]);
 for(const [path,digest] of Object.entries(phase8.compatibilityFiles))if(!path.endsWith('.css'))expect(hash(read(path)),path).toBe(current.afterHashes[path]??phase13.frontend.afterHashes[path]??phase10.frontendSuccessor[path]??digest);
 expect(m.after.inline).toBe(m.before.inline);expect(m.proofs.containment.selectors).toBeGreaterThan(0);
});
