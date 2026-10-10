// @vitest-environment node
import { expect, it } from 'vitest';
// @ts-expect-error Node-only owned inventory fixture.
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only owned inventory fixture.
import { join, resolve } from 'node:path';
// @ts-expect-error Existing static inventory, not browser code.
import { styleInventory } from '../../scripts/style-boundary-guard.mjs';
function fixture(check: (root: string) => void) {
  const workspace=resolve('.'),root=mkdtempSync(join(workspace,'.codex-style-inventory-'));
  try {
    mkdirSync(join(root,'src'));
    writeFileSync(join(root,'src/fixture.css'),'.alpha .beta,.alpha:hover,.beta {color:red;}');
    writeFileSync(join(root,'src/a.tsx'),'const A = <div className="alpha_suffix" style={{fontSize:12}} />;');
    writeFileSync(join(root,'src/b.tsx'),'const B = <div className="beta" style={dynamic} />;');
    check(root);
  } finally {
    if(!root.startsWith(workspace+'/')&&!root.startsWith(workspace+'\\'))throw Error('outside owned fixture');
    if(!root.includes('.codex-style-inventory-'))throw Error('unexpected fixture');
    rmSync(root,{recursive:true,force:true});
  }
}
it('retains literal substring association, stable source order and inline classification',()=>fixture(root=>{
  const value=styleInventory(root,true);
  expect(value.rows.map((r:{callers:string[]})=>r.callers)).toEqual([['src/a.tsx','src/b.tsx'],['src/a.tsx'],['src/b.tsx']]);
  expect(value.inline.map((r:{decision:string})=>r.decision)).toEqual(['DEFER','KEEP']);
  expect(value.metrics.inline).toBe(2);
}));
it('never carries caller associations across changed live bytes or projections; restored bytes restore inventory',()=>fixture(root=>{
  const original=styleInventory(root,true);
  writeFileSync(join(root,'src/a.tsx'),'const A = <div className="gamma_suffix" style={{fontSize:12}} />;');
  expect(styleInventory(root,true).rows[1].callers).toEqual([]);
  writeFileSync(join(root,'src/a.tsx'),'const A = <div className="alpha_suffix" style={{fontSize:12}} />;');
  expect(styleInventory(root,true)).toEqual(original);
  const projection={addedPaths:[],read:(path:string)=>path.endsWith('.css')?'.beta {color:red;}':'const P = <div className="none" />;'};
  expect(styleInventory(root,true,projection).rows[0].callers).toEqual([]);
  expect(styleInventory(root,true)).toEqual(original);
}));
