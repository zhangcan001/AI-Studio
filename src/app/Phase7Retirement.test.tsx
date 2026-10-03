// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
// @ts-expect-error Node helpers are test-only, matching repository configuration.
import { existsSync, readFileSync, readdirSync } from "node:fs";
// @ts-expect-error Node helpers are test-only.
import { execFileSync } from "node:child_process";
// @ts-expect-error Node helpers are test-only.
import { resolve, dirname } from "node:path";
// @ts-expect-error Node helpers are test-only.
import { createHash } from "node:crypto";
import ts from "typescript";
import { ShellHost } from "./ShellHost";
import { fromLegacyLocation } from "./routes/legacyAdapter";
import { parseRoute, resolveResume, writeRouteResume, readRouteResume } from "./routes/resumeAdapter";
import { appRouteReducer, initialRouteState } from "./routes/reducer";
import type { AppRoute } from "./routes/types";
import { normalCreate } from "../features/create/createModel";
import { normalRuns } from "../features/runs/runsModel";
import { normalLibrary } from "../features/library/libraryModel";
import { WORKSPACES } from "../types/workspaceResume";
const read=(p:string)=>readFileSync(p,"utf8");
const matrix=JSON.parse(read("docs/architecture/phase7-retirement-matrix.json")) as {rows:{entry:string;file:string;decision:string;replacement:string;compatibility:string;status:string}[];frozenSources:Record<string,{files:number;sha256:string}>};
const app=()=>read("src/app/App.tsx");
const project="phase7-project";
const create:AppRoute={kind:"create",projectId:project,shotId:"shot-exact",stage:"video"};
afterEach(()=>{cleanup();localStorage.clear();});
function files(dir:string):string[]{return readdirSync(dir,{withFileTypes:true}).flatMap((e:{name:string;isDirectory:()=>boolean})=>e.isDirectory()?files(`${dir}/${e.name}`):/\.(tsx?|mjs)$/.test(e.name)?[`${dir}/${e.name}`]:[]);}
it("phase7_target1 retirement matrix accounts for every candidate",()=>{
 for(const entry of ["legacy shell","workflows","command-center","studio","video","shots","assets","prompts","tasks","production run","production drawer","production review","production queue","direct-generation","task state","profile editor","reference editor"]){expect(matrix.rows.find(r=>r.entry===entry)).toBeTruthy();}
 for(const row of matrix.rows){expect(["DELETE_NOW","KEEP_ADVANCED","KEEP_COMPAT","KEEP_SHARED"]).toContain(row.decision);expect(row.replacement).toBeTruthy();expect(row.compatibility).toBeTruthy();}
});
it("phase7_target2 ignores old shell preference and preserves canonical navigation",()=>{
 localStorage.setItem("aistudio.shellMode","legacy");const navigate=vi.fn();
 render(<ShellHost route={create} projectSelector={<select aria-label="项目"/>} navigate={navigate} back={vi.fn()}><p>canonical content</p></ShellHost>);
 expect(document.querySelector(".v3-shell")).toBeTruthy();expect(document.querySelector(".studio-shell")).toBeNull();expect(screen.queryByText("返回旧版导航")).toBeNull();expect(screen.queryByText("试用项目导航")).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"运行"}));expect(navigate).toHaveBeenCalledWith({kind:"runs",projectId:project});expect(localStorage.getItem("aistudio.shellMode")).toBe("legacy");
});
it("phase7_target3 all eleven legacy locators have canonical destinations",()=>{
 const expected:Record<string,string>={"command-center":"project",studio:"create",video:"create",shots:"create",assets:"library",prompts:"library",tools:"system-settings",tasks:"runs",projects:"project-settings",workflows:"project-settings",settings:"system-settings"};
 expect(Object.keys(expected)).toHaveLength(11);
 for(const workspace of WORKSPACES){const route=fromLegacyLocation({workspace,projectId:project});expect(route.kind).toBe(expected[workspace]);expect(parseRoute(route)).toBeTruthy();}
});
it("phase7_target4 normal Create never renders batch or shot legacy UI",()=>{
 expect(normalCreate(fromLegacyLocation({workspace:"shots",projectId:project,shotId:"s"}))).toBe(true);
 for(const workspace of ["studio","video"] as const){const r=fromLegacyLocation({workspace,projectId:project});expect(r).toMatchObject({kind:"create",surface:"batch",stage:workspace==="studio"?"image":"video"});expect(normalCreate(r)).toBe(false);}
 expect(app()).toContain('normalCreate(route) && route.kind === "create"');expect(app()).toContain('["advanced-shots", "advanced-production", "advanced-review"].includes(route.section)');
});
it("phase7_target5 task production review locators use Runs and preserve exact context",()=>{
 for(const workspace of ["tasks","shots"] as const)for(const section of ["production","review"] as const){const r=fromLegacyLocation({workspace,section,projectId:project,taskId:"t",batchId:"b",itemId:"i",reviewId:"r",shotId:"s",assetId:"a",stage:"VIDEO"});expect(normalRuns(r)).toBe(true);expect(r).toMatchObject({run:{source:"queue-batch",id:"b"},context:{reviewId:"r",itemId:"i",taskId:"t",batchId:"b",assetId:"a",shotId:"s"}});expect(parseRoute(r)).toEqual(r);}
 const r=fromLegacyLocation({workspace:"tasks",projectId:project,taskId:"exact"});expect(r).toMatchObject({run:{source:"task",id:"exact"}});expect(normalRuns(r)).toBe(true);
});
it("phase7_target6 asset prompt routes are normal Library with explicit advanced opt in",()=>{
 const asset=fromLegacyLocation({workspace:"assets",projectId:project,assetId:"exact"});expect(asset).toMatchObject({kind:"library",resource:{kind:"asset",id:"exact"}});expect(normalLibrary(asset)).toBe(true);
 expect(fromLegacyLocation({workspace:"prompts",projectId:project})).toMatchObject({kind:"library",filter:"prompts"});
 for(const filter of ["advanced-assets","advanced-prompts"])expect(normalLibrary({kind:"library",projectId:project,filter})).toBe(false);
});
it("phase7_target7 old workflow routes reach the real Lab not a wrapper",()=>{
 for(const projectId of [undefined,project])expect(fromLegacyLocation({workspace:"workflows",projectId})).toMatchObject({section:"advanced-workflows"});
 expect(app()).toContain('<WorkflowLabPage');expect(app()).not.toContain('import("../features/workflows/WorkflowWorkspace")');expect(existsSync("src/features/workflows/WorkflowWorkspace.tsx")).toBe(false);expect(existsSync("src/services/workflowClient.ts")).toBe(true);
});
it("phase7_target8 command center defaults to Overview and retains explicit analysis",()=>{
 expect(fromLegacyLocation({workspace:"command-center",projectId:project})).toEqual({kind:"project",projectId:project,page:"overview"});expect(fromLegacyLocation({workspace:"command-center",section:"analysis",projectId:project})).toEqual({kind:"project-settings",projectId:project,section:"advanced-project"});expect(fromLegacyLocation({workspace:"projects",projectId:project})).toMatchObject({kind:"project-settings",section:"general"});
});
it("phase7_target9 deleted components have zero static dynamic or reexport callers",()=>{
 const retired=matrix.rows.filter(r=>r.decision==="DELETE_NOW").map(r=>resolve(r.file));for(const file of retired)expect(existsSync(file)).toBe(false);
 const violations:string[]=[];
 for(const file of files("src")){const ast=ts.createSourceFile(file,read(file),ts.ScriptTarget.Latest,true);function visit(n:ts.Node){let spec:ts.Node|undefined;if(ts.isImportDeclaration(n)||ts.isExportDeclaration(n))spec=n.moduleSpecifier;else if(ts.isCallExpression(n)&&n.expression.kind===ts.SyntaxKind.ImportKeyword)spec=n.arguments[0];if(spec&&ts.isStringLiteral(spec)&&spec.text.startsWith('.')){const base=resolve(dirname(file),spec.text);if(retired.some(p=>[base,base+'.ts',base+'.tsx',base+'.css',base+'/index.ts'].includes(p)))violations.push(`${file}:${spec.text}`);}ts.forEachChild(n,visit);}visit(ast);}
 expect(violations).toEqual([]);
});
it("phase7_target10 runtime dispatch cannot resurrect retired screens",()=>{
 expect(app()).not.toMatch(/shellMode|readShellMode|SHELL_MODE_KEY|<WorkflowWorkspace|<StudioShell/);expect(read("src/app/ShellHost.tsx")).not.toMatch(/localStorage|StudioShell|onModeChange/);
 expect(app()).not.toMatch(/workspace === "(?:shots|tasks|workflows)" &&/);expect(read("src/app/routes/legacyAdapter.ts")).toContain("fromLegacyLocation");
});
it("phase7_target11 Advanced capabilities remain explicitly reachable",()=>{
 for(const marker of ['route.surface === "batch"','route.filter === "advanced-assets"','route.filter === "advanced-prompts"','route.section === "advanced-tools"','route.section === "advanced-workflows"','["advanced-shots", "advanced-production", "advanced-review"].includes(route.section)','route.section === "advanced-tasks"','route.section === "advanced-project"'])expect(app()).toContain(marker);
 expect(read("src/features/workflow-lab/WorkflowLabPage.tsx")).toContain("LabDiagnosticsPane");for(const r of matrix.rows.filter(r=>r.decision==="KEEP_ADVANCED"))expect(existsSync(r.file)).toBe(true);
});
it("phase7_target12 Product and Advanced transport guards remain intact",()=>{
 const output=execFileSync("node",["scripts/dev088-architecture-guard.mjs"],{encoding:"utf8"});expect(output).toContain("FRONTEND_NO_RAW_INVOKE=PASS");
 for(const dir of ["src/features/create","src/features/runs","src/features/library"]){for(const file of files(dir).filter(f=>!f.includes('.test.')))expect(read(file)).not.toMatch(/from ["'][^"']*(?:tauriClient|services\/ipc)["']|@tauri-apps\/api/);}
},20000);
it("phase7_target13 invariant tests survive UI retirement at actual owners",()=>{
 for(const file of ["src/features/workflows/WorkflowQuickTestQueue.test.tsx","src/features/workflows/WorkflowRecipePromotionUat.test.tsx","src/features/workflows/WorkflowVersionDelete.test.tsx","src/features/workflows/WorkflowPurgeUat.test.tsx","src/features/tasks/TaskHistoryDetail.queueSubmission.test.tsx","src/features/runs/RunsPage.test.tsx","src/features/generators/GeneratorSettingsPage.test.tsx"]){expect(read(file).length).toBeGreaterThan(1000);expect(read(file)).not.toMatch(/\bit\.skip\(|\bdescribe\.skip\(/);}
 const guard=read("src-tauri/tests/production_execution_authority_boundary.rs");expect(guard).toContain("useWorkflowLabController.ts");expect(guard).toContain("startProductionQueue");
});
it("phase7_target14 old new resume Back and project switch retain isolation",()=>{
 const legacy={lastProjectId:project,lastWorkspace:"shots" as const,lastShotId:"s"};expect(resolveResume(undefined,legacy,[project])).toMatchObject({kind:"create",shotId:"s"});writeRouteResume(create);expect(resolveResume(readRouteResume(),legacy,[project])).toEqual(create);
 const lab:AppRoute={kind:"system-settings",section:"advanced-workflows",returnTo:create};let s=appRouteReducer(initialRouteState,{type:"restore",route:lab});expect(appRouteReducer(s,{type:"back"}).current).toEqual(create);s=appRouteReducer(s,{type:"switch-project",projectId:"other"});expect(s.current).toEqual({kind:"project",projectId:"other",page:"overview"});expect(s.history).toEqual([]);
 const asset:AppRoute={kind:"library",projectId:project,resource:{kind:"asset",id:"asset"},filter:"images"};let library=appRouteReducer(initialRouteState,{type:"restore",route:asset});library=appRouteReducer(library,{type:"navigate",route:{...asset,filter:"advanced-assets"}});expect(appRouteReducer(library,{type:"back"}).current).toEqual(asset);const resumed=appRouteReducer(initialRouteState,{type:"restore",route:{...asset,filter:"advanced-assets"}});expect(appRouteReducer(resumed,{type:"back"}).current).toEqual({...asset,filter:"all"});
 expect(read("src/app/routes/useAppRoute.ts")).toContain("writeRouteResume(state.current)");expect(app()).not.toContain("saveWorkspaceResume(");
});
it("phase7_target15 keyboard skip focus and active navigation survive single shell",()=>{
 const navigate=vi.fn();render(<ShellHost route={create} projectSelector={null} navigate={navigate} back={vi.fn()}><p>content</p></ShellHost>);expect(screen.getByRole("button",{name:"创作"}).getAttribute("aria-current")).toBe("page");expect(screen.getByRole("link",{name:"跳到主要内容"}).getAttribute("href")).toBe("#v3-main");document.getElementById("v3-main")!.focus();expect(document.activeElement?.id).toBe("v3-main");fireEvent.keyDown(window,{key:"k",ctrlKey:true});fireEvent.keyDown(window,{key:"K",metaKey:true});expect(navigate).toHaveBeenCalledTimes(2);expect(navigate).toHaveBeenLastCalledWith({kind:"create",projectId:project,stage:"image"});
});
it("phase7_target16 frozen migration domain and CSS boundaries are unchanged",()=>{
 expect(readdirSync("src-tauri/migrations").some((f:string)=>f.startsWith("043"))).toBe(false);expect(read("src-tauri/src/application/project_backup_service.rs")).toContain("const BACKUP_VERSION: u32 = 20;");
 function allFiles(dir:string):string[]{return readdirSync(dir,{withFileTypes:true}).flatMap((e:{name:string;isDirectory:()=>boolean})=>e.isDirectory()?allFiles(`${dir}/${e.name}`):[`${dir}/${e.name}`]).sort();}
 for(const [root,expected] of Object.entries(matrix.frozenSources)){const list=root.endsWith('.css')?[root]:allFiles(root);expect(list).toHaveLength(expected.files);const text=list.map(f=>f+'\n'+read(f).replaceAll('\r\n','\n')).join('\n');expect(createHash('sha256').update(text).digest('hex')).toBe(expected.sha256);}

});

