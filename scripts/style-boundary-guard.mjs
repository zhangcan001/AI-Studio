// Phase9 extends DEV-088; PostCSS is Vite's existing parser, not a new dependency.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import ts from 'typescript';
const require = createRequire(import.meta.url);
export const postcss = createRequire(require.resolve('vite'))('postcss');
export const read = p => readFileSync(p, 'utf8').replaceAll('\r\n', '\n');
export const hash = s => createHash('sha256').update(s).digest('hex');
import { phase13Boundary } from './phase13-observability-guard.mjs';
import { m1ParentReader } from './m1-readiness-boundary-guard.mjs';
export function files(root) {
  return readdirSync(root, { withFileTypes:true }).flatMap(e => e.isDirectory() ? files(`${root}/${e.name}`) : [`${root}/${e.name}`]).sort();
}
const compact = s => s.replace(/\s+/g, ' ').trim();
export const context = r => { const c=[]; for(let n=r.parent;n?.type!=='root';n=n?.parent) if(n?.type==='atrule')c.unshift(`@${n.name} ${compact(n.params)}`); return c.join(' / '); };
// Functional selectors are balanced; :where contributes zero, :is/:not/:has use max.
export function specificity(s) {
  let extra=[0,0,0];
  s=s.replace(/"[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*'/g,'""');
  let m;
  while((m=/:([\w-]+)\(/.exec(s))) {
    let end=m.index+m[0].length,depth=1;
    for(;end<s.length&&depth;end++){if(s[end]==='(')depth++;if(s[end]===')')depth--;}
    const inner=s.slice(m.index+m[0].length,end-1),name=m[1];
    const candidates=postcss.list.comma(inner).map(specificity).sort((a,b)=>a[0]-b[0]||a[1]-b[1]||a[2]-b[2]);
    const add=name==='where'?[0,0,0]:['is','not','has'].includes(name)?candidates.at(-1)||[0,0,0]:[0,1,0];
    extra=extra.map((v,i)=>v+add[i]);s=s.slice(0,m.index)+' '+s.slice(end);
  }
  const ids=(s.match(/#[\w-]+/g)||[]).length;
  const attrs=(s.match(/\[[^\]]*\]/g)||[]).length;s=s.replace(/\[[^\]]*\]/g,' ');
  const pseudoElements=(s.match(/::[\w-]+/g)||[]).length;s=s.replace(/::[\w-]+/g,' ');
  const classes=(s.match(/\.[\w-]+|:[\w-]+/g)||[]).length;s=s.replace(/#[\w-]+|\.[\w-]+|:[\w-]+/g,' ');
  const types=(s.match(/(?:^|[\s>+~])([a-zA-Z][\w-]*)/g)||[]).length;
  return [extra[0]+ids,extra[1]+attrs+classes,extra[2]+types+pseudoElements];
}
export const globalTag = s => !/[.#\[]/.test(s) && !s.includes(':where(');
export const deep = s => specificity(s)[1]>=5 || s.replace(/\([^)]*\)/g,'').split(/[>+~]|\s+/).filter(Boolean).length>=5;
export function styleInventory(root='.', detailed=true) {
  const all=files(`${root}/src`).map(p=>p.slice(root.length+1));
  const sources=detailed?all.filter(p=>/\.tsx?$/.test(p)&&!p.includes('.test.')).map(p=>({path:p,text:read(`${root}/${p}`)})):[];
  const rows=[],inline=[];const metrics={files:0,lines:0,selectors:0,global:0,important:0,ids:0,inline:0,variables:0,media:0};
  for(const {path,text} of sources) {
    const ast=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true);
    function visit(n){if(ts.isJsxAttribute(n)&&n.name.getText(ast)==='style'){const expression=n.initializer?.expression;const fixed=expression&&ts.isObjectLiteralExpression(expression)&&expression.properties.every(p=>ts.isPropertyAssignment(p)&&(ts.isStringLiteral(p.initializer)||ts.isNumericLiteral(p.initializer)));inline.push({path,line:ast.getLineAndCharacterOfPosition(n.pos).line+1,decision:fixed?'DEFER':'KEEP',reason:fixed?'Isolated fixed inline style; no mechanical component rewrite.':'Runtime expression; preserve dynamic authority.'});}ts.forEachChild(n,visit);}visit(ast);
  }
  for(const path of all.filter(p=>/\.(css|scss)$/.test(p))) {
    const text=read(`${root}/${path}`),ast=postcss.parse(text,{from:path});metrics.files++;metrics.lines+=text.trimEnd().split('\n').length;
    ast.walkAtRules('media',()=>metrics.media++);
    ast.walkDecls(d=>{if(d.important)metrics.important++;if(d.prop.startsWith('--'))metrics.variables++;});
    ast.walkRules(r=>{for(const selector of r.selectors){metrics.selectors++;if(globalTag(selector))metrics.global++;if(specificity(selector)[0])metrics.ids++;
      const classes=[...selector.matchAll(/\.([\w-]+)/g)].map(m=>m[1]);
      // Candidate references, not a dead-code oracle: templates/conditional strings included.
      const callers=sources.filter(s=>classes.some(c=>s.text.includes(c))).map(s=>s.path);
      rows.push({path,selector_or_scope:selector,context:context(r),owner:path.includes('/features/')?path.split('/features/')[1].split('/')[0]:path.includes('/v3/')?'V3 shell':'shared/compatibility',usage_count:callers.length,callers,global_scope:globalTag(selector),specificity:specificity(selector),duplicate_group:null,legacy:selector.includes('studio-shell'),decision:globalTag(selector)?'KEEP_GLOBAL':path==='src/app/App.css'?'DEFER':'KEEP',replacement:null,risk:'Retain unless exact ownership/deletion proof is recorded.',tests:['Phase9 style guard','Native before/after responsive capture']});
    }});
  }
  metrics.inline=inline.length;return {metrics,rows,inline};
}
export function debt(rows,root='.') {
  const budgets={},importance=new Map();
  for(const path of new Set(rows.map(r=>r.path))){postcss.parse(read(`${root}/${path}`)).walkRules(r=>{for(const s of r.selectors){const key=`${path}|${context(r)}|${s}`;importance.set(key,(importance.get(key)||0)+(r.nodes||[]).filter(n=>n.type==='decl'&&n.important).length);}});}
  for(const row of rows){const key=`${row.path}|${row.context}|${row.selector_or_scope}`;const counts={important:0,id:specificity(row.selector_or_scope)[0],deep:deep(row.selector_or_scope)?1:0,global:globalTag(row.selector_or_scope)?1:0};
    counts.important=importance.get(key)||0;
    if(Object.values(counts).some(Boolean))budgets[key]=counts;
  }return budgets;
}
export function debtViolations(actual,allowed) {
  const errors=[];for(const [key,counts] of Object.entries(actual))for(const [kind,count] of Object.entries(counts))if(count>(allowed[key]?.[kind]||0))errors.push(`${kind}:${key}`);return errors;
}
// Flatten only local @imports and preserve order; cycles and late imports are invalid.
export function expandedCss(path,stack=[]) {
  path=path.replaceAll('\\','/');if(stack.includes(path))throw Error(`CSS import cycle: ${path}`);
  const ast=postcss.parse(read(path),{from:path});let ordinary=false;
  for(const n of [...ast.nodes]){if(n.type==='comment')continue;if(n.type==='atrule'&&n.name==='import'){if(ordinary)throw Error(`Late CSS import: ${path}`);const m=/^["']([^"']+)["']\s*$/.exec(n.params);if(!m)throw Error('Only explicit local CSS imports allowed');const child=resolve(dirname(path),m[1]).replaceAll('\\','/');n.replaceWith(...expandedCss(child,[...stack,path]).nodes);}else ordinary=true;}return ast;
}
export function canonical(ast) {
  const output=[];
  ast.walkRules(r=>output.push({context:context(r),selectors:r.selectors.map(compact),declarations:r.nodes.filter(n=>n.type==='decl').map(n=>[n.prop,compact(n.value),!!n.important])}));
  return hash(JSON.stringify(output));
}
// Later measured behavior work must explicitly chain from the frozen digest.
// This does not authorize CSS, new files, transport changes or debt expansion.
export function performanceSuccessor(snapshot, review) {
  const next={...snapshot},violations=[],seen=new Set();
  for(const seam of review?.optimizations||[]) {
    if(seam.status!=='MEASURED_VERIFIED')continue;
    const path=seam.path,proof=seam.sourceFreeze;
    if(seen.has(path)||!/^src\/(?:features|app)\/.+\.tsx?$/.test(path)||
      !Object.hasOwn(snapshot,path)||proof?.beforeHash!==snapshot[path]||
      !/^[a-f0-9]{64}$/.test(proof?.afterHash||'')||
      seam.baseline?.samples<5||seam.after?.samples<5||
      !Number.isFinite(seam.baseline?.samples)||!Number.isFinite(seam.after?.samples)) {
      violations.push(`unreviewed-performance-successor:${path}`);continue;
    }
    seen.add(path);next[path]=proof.afterHash;
  }
  return {snapshot:next,violations};
}
export function styleBoundary(root,manifest) {
  const inv=styleInventory(root,false),violations=debtViolations(debt(inv.rows,root),manifest.grandfathered);
  for(const r of inv.rows){const match=manifest.ownedFiles[r.path];if(match&&(!match.some(prefix=>r.selector_or_scope.startsWith(prefix))||/\.(?:runs?|create|library|asset|project|prompt-studio)-/.test(r.selector_or_scope)))violations.push(`ownership:${r.path}:${r.selector_or_scope}`);if(r.selector_or_scope.includes('.studio-shell'))violations.push(`retired-root:${r.path}`);}
  // Phase10 authorizes only reviewed seam paths; CSS and every other source stay frozen.
  const successorPath=`${root}/docs/architecture/phase10-frontend-architecture.json`;
  const successor=existsSync(successorPath)?JSON.parse(read(successorPath)):undefined;
  const allowed=new Set(successor?.seams.flatMap(s=>s.paths)||[]);
  const scoped=successor?.frontendSuccessor||{};
  for(const path of Object.keys(scoped))if(!allowed.has(path))violations.push(`unscoped-successor:${path}`);
  const priorSnapshot={...manifest.productionFrontendSnapshot,...Object.fromEntries(Object.entries(scoped).filter(([p])=>allowed.has(p)))};
  const performancePath=`${root}/docs/architecture/phase12-performance.json`;
  const performanceReview=existsSync(performancePath)?JSON.parse(read(performancePath)):undefined;
  const reviewed=performanceSuccessor(priorSnapshot,performanceReview);
  violations.push(...reviewed.violations);
  const productionSnapshot=reviewed.snapshot;
  const phase13Path=`${root}/docs/architecture/phase13-observability.json`;
  if(existsSync(phase13Path)) {
    const phase13=JSON.parse(read(phase13Path));
    const phase8=JSON.parse(read(`${root}/docs/architecture/phase8-backend-decomposition.json`));
    const phase13Result=phase13Boundary(root,phase13,performanceReview,phase8.backendSourceSnapshot);
    violations.push(...phase13Result.violations);
    Object.assign(productionSnapshot,phase13.frontend.afterHashes);
  }
  const checkpoint=m1ParentReader(root);
  violations.push(...checkpoint.violations);
  if(!checkpoint.violations.length)for(const [path,digest] of Object.entries(checkpoint.afterHashes))if(/^src\/.+\.tsx?$/.test(path))productionSnapshot[path]=digest;
  for(const [path,digest] of Object.entries(productionSnapshot))if(hash(read(`${root}/${path}`))!==digest)violations.push(`behavior:${path}`);
  for(const path of files(`${root}/src`).map(p=>p.slice(root.length+1)).filter(p=>/\.tsx?$/.test(p)&&!p.includes('.test.')))if(!(path in productionSnapshot))violations.push(`new-production-source:${path}`);
  // Only a validated successor may advance this existing stylesheet; historical
  // manifests and every other CSS boundary stay frozen. No new styles exemption.
  const currentStyles={...manifest.styleSnapshots};
  if(!checkpoint.violations.length && checkpoint.afterHashes['src/features/library/LibraryPage.css'])currentStyles['src/features/library/LibraryPage.css']=checkpoint.afterHashes['src/features/library/LibraryPage.css'];
  for(const [path,digest] of Object.entries(currentStyles))if(hash(read(`${root}/${path}`))!==digest)violations.push(`unreviewed-style:${path}`);
  for(const path of files(`${root}/src`).map(p=>p.slice(root.length+1)).filter(p=>/\.(css|scss)$/.test(p)))if(!(path in manifest.styleSnapshots))violations.push(`unreviewed-style:${path}`);
  return {violations,inventory:inv};
}
