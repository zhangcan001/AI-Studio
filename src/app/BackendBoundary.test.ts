// @vitest-environment node
import { describe, it, expect } from "vitest";
// @ts-expect-error Node helpers are used only by this source-boundary test.
import { readFileSync } from "node:fs";
// @ts-expect-error Node helpers are used only by this source-boundary test.
import { createHash } from "node:crypto";
// @ts-expect-error Build-time architecture module, not browser production code.
import { backendPerformanceBoundary, backendBoundary, productionRust, sqlFootprint, rustFiles } from "../../scripts/backend-boundary-guard.mjs";
// @ts-expect-error Build-time successor validator, never production browser code.
import { phase13Boundary } from "../../scripts/phase13-observability-guard.mjs";
// @ts-expect-error Build-time current successor, not application code.
import { m1ParentReader } from "../../scripts/m1-readiness-boundary-guard.mjs";
// @ts-expect-error Build-time reviewed additive tag command, not browser production code.
import { m3LibraryParentReader } from "../../scripts/m3-library-findability-boundary-guard.mjs";
const read=(p:string):string=>readFileSync(p,"utf8");
const manifest=JSON.parse(read("docs/architecture/phase8-backend-decomposition.json"));
const phase13=JSON.parse(read("docs/architecture/phase13-observability.json"));
const phase12=JSON.parse(read("docs/architecture/phase12-performance.json"));
describe("Phase8 backend compatibility",()=>{
it("rejects widened observability scope, missing privacy guarantees, incorrect counts and weakened parent proof",()=>{
 const cases = [
  { mutate: (r: typeof phase13) => { r.frontend.existingPaths.push("src/product/client.ts"); }, code: "phase13-frontend-scope-mismatch" },
  { mutate: (r: typeof phase13) => { r.privacy.promptExport = true; }, code: "phase13-privacy-invariants-missing" },
  { mutate: (r: typeof phase13) => { r.backend.afterFiles++; }, code: "phase13-backend-parent-or-file-count-changed" },
 ];
 for (const { mutate, code } of cases) {
  const reviewed = structuredClone(phase13); mutate(reviewed);
  expect(phase13Boundary(".", reviewed, phase12, manifest.backendSourceSnapshot).violations).toContain(code);
 }
 const parent = structuredClone(phase12); parent.backendOptimization2.after.samples = 0;
 expect(phase13Boundary(".", phase13, parent, manifest.backendSourceSnapshot).violations).toContain("phase12-measurement-proof-invalid");
}, 30000);
it("target1 prevents production SQL growth while recognizing only real test scopes",()=>{
 expect(backendBoundary(".",manifest).violations).toEqual([]);
 const fixture='// sqlx::query("SELECT x FROM x")\n#[cfg(test)] mod tests {fn f(){sqlx::query("SELECT y FROM y");}}\nfn real(){ sqlx :: query("SELECT z FROM z"); }';
 expect(sqlFootprint(fixture)).toEqual({sqlx:1,pools:0,sql:1});
 expect(sqlFootprint('use sqlx::{SqlitePool as Renamed};')).toMatchObject({sqlx:1,pools:1});
 expect(sqlFootprint('fn real(){let q = r###"PRAGMA foreign_keys"###;}')).toMatchObject({sql:1});
 expect(sqlFootprint('/* outer /* sqlx:: */ nested */ fn clean() {}')).toEqual({sqlx:0,pools:0,sql:0});
 expect(productionRust('#[cfg(test)] fn test(){let x="}";} fn live(){sqlx::query("SELECT x FROM x");}')).toContain('fn live()');
 expect(sqlFootprint('#[cfg(not(test))] fn real(){sqlx::query("SELECT x FROM x");}').sqlx).toBe(1);
 expect(manifest.rows.every((r:{decision:string})=>!["UNKNOWN","UNCLASSIFIED"].includes(r.decision))).toBe(true);
});
it("target2 preserves registered IPC signatures and frozen Phase7 consumers",()=>{
 const commands=rustFiles("src-tauri/src/commands").flatMap((p:string)=>[...read(p).matchAll(/#\[tauri::command[^\]]*\]\s*pub\s+async\s+fn\s+(\w+)[\s\S]*?(?=\{)/g)].map(x=>({name:x[1],signature:x[0].replace(/\s+/g," ").trim()}))).sort((a:{name:string},b:{name:string})=>a.name.localeCompare(b.name));
 const successor=m3LibraryParentReader(".");expect(successor.violations).toEqual([]);
 expect(commands).toEqual([...manifest.ipcContracts,...phase13.commands.addedSignatures,...successor.addedCommandSignatures].sort((a:{name:string},b:{name:string})=>a.name.localeCompare(b.name)));
 const backend=rustFiles("src-tauri/src").map((p:string)=>p.replace(/\\/g,"/")).sort();
 const reviewed=backendPerformanceBoundary(".",manifest.backendSourceSnapshot,phase12,phase13);
 expect(reviewed.violations).toEqual([]);
 expect(createHash("sha256").update(backend.map((p:string)=>p+'\n'+read(p).replace(/\r\n/g,"\n")).join('\n')).digest("hex")).toBe(reviewed.sha256);
  expect(manifest.rows.map((r:{path:string})=>r.path).sort()).toEqual(backend.filter((p:string)=>!phase13.backend.addedPaths.includes(p)&&!successor.addedPaths.includes(p)));
});
it("target2 preserves frozen Phase7 consumers through the validated successor",()=>{
 // Phase10 advances only explicitly scoped frontend seam pins; backend/IPC remain exact.
 const phase9=JSON.parse(read("docs/architecture/phase9-style-cleanup.json"));
 const phase10=JSON.parse(read("docs/architecture/phase10-frontend-architecture.json"));
 const current=m1ParentReader('.');expect(current.violations).toEqual([]);
 for(const [path,digest] of Object.entries(manifest.compatibilityFiles))expect(createHash("sha256").update(read(path).replace(/\r\n/g,"\n")).digest("hex"),path).toBe(current.afterHashes[path]??phase13.frontend.afterHashes[path]??(path==='src/app/App.css'?phase9.styleSnapshots[path]:phase10.frontendSuccessor[path]??digest));
 expect(read('src-tauri/src/application/project_backup_service.rs')).toContain('const BACKUP_VERSION: u32 = 20;');
});
});
