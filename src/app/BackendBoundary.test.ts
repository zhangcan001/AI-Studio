// @vitest-environment node
import { describe, it, expect } from "vitest";
// @ts-expect-error Node helpers are used only by this source-boundary test.
import { readFileSync } from "node:fs";
// @ts-expect-error Node helpers are used only by this source-boundary test.
import { createHash } from "node:crypto";
// @ts-expect-error Build-time architecture module, not browser production code.
import { backendPerformanceBoundary, backendBoundary, productionRust, sqlFootprint, rustFiles } from "../../scripts/backend-boundary-guard.mjs";
const read=(p:string):string=>readFileSync(p,"utf8");
const manifest=JSON.parse(read("docs/architecture/phase8-backend-decomposition.json"));
describe("Phase8 backend compatibility",()=>{
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
 expect(commands).toEqual(manifest.ipcContracts);
 const backend=rustFiles("src-tauri/src").map((p:string)=>p.replace(/\\/g,"/")).sort();
 expect(backend).toHaveLength(manifest.backendSourceSnapshot.files);
 const reviewed=backendPerformanceBoundary(".",manifest.backendSourceSnapshot,JSON.parse(read("docs/architecture/phase12-performance.json")));
 expect(reviewed.violations).toEqual([]);
 expect(createHash("sha256").update(backend.map((p:string)=>p+'\n'+read(p).replace(/\r\n/g,"\n")).join('\n')).digest("hex")).toBe(reviewed.sha256);
 expect(manifest.rows.map((r:{path:string})=>r.path).sort()).toEqual(backend);
 // Phase10 advances only explicitly scoped frontend seam pins; backend/IPC remain exact.
 const phase9=JSON.parse(read("docs/architecture/phase9-style-cleanup.json"));
 const phase10=JSON.parse(read("docs/architecture/phase10-frontend-architecture.json"));
 for(const [path,digest] of Object.entries(manifest.compatibilityFiles))expect(createHash("sha256").update(read(path).replace(/\r\n/g,"\n")).digest("hex"),path).toBe(path==='src/app/App.css'?phase9.styleSnapshots[path]:phase10.frontendSuccessor[path]??digest);
 expect(read('src-tauri/src/application/project_backup_service.rs')).toContain('const BACKUP_VERSION: u32 = 20;');
});
});
