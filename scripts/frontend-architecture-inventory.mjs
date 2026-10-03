// Read-only AST inventory. Metrics are candidates, not proof of duplicate authority.
import ts from 'typescript';
import { resolve, dirname, relative } from 'node:path';
import { files, read } from './style-boundary-guard.mjs';
export function frontendInventory(root='.') {
  const paths=files(`${root}/src`).filter(p=>/\.tsx?$/.test(p));
  const rows=[];
  for(const absolute of paths) {
    const path=relative(root,absolute).replaceAll('\\','/'), source=read(absolute);
    const ast=ts.createSourceFile(path,source,ts.ScriptTarget.Latest,true);
    const test=path.includes('.test.'), feature=path.match(/src\/features\/([^/]+)/)?.[1];
    const imports=[],calls=[],symbols=[];let propDepth=0;
    const counts={useState:0,useEffect:0,useLayoutEffect:0,useMemo:0,useCallback:0,contexts:0,localStorage:0,sessionStorage:0,listeners:0,timers:0,subscriptions:0};
    function visit(n) {
      if(ts.isImportDeclaration(n)&&ts.isStringLiteral(n.moduleSpecifier))imports.push(n.moduleSpecifier.text);
      if(ts.isFunctionDeclaration(n)&&n.name)symbols.push(n.name.text);
      if(ts.isCallExpression(n)) {
        const name=n.expression.getText(ast);calls.push(name);
        if(name in counts)counts[name]++;
        if(name==='createContext')counts.contexts++;
        if(/(?:^|\.)localStorage\./.test(name))counts.localStorage++;
        if(/(?:^|\.)sessionStorage\./.test(name))counts.sessionStorage++;
        if(/\.addEventListener$/.test(name))counts.listeners++;
        if(/(?:^|\.)(?:setTimeout|setInterval)$/.test(name))counts.timers++;
        if(/(?:subscribe\w*|\.subscribe|^listen)$/.test(name))counts.subscriptions++;
      }
      if(ts.isPropertyAccessExpression(n)) { let depth=1,p=n.expression;while(ts.isPropertyAccessExpression(p)){depth++;p=p.expression;}propDepth=Math.max(propDepth,depth); }
      ts.forEachChild(n,visit);
    }visit(ast);
    const resolved=imports.map(i=>i.startsWith('.')?relative(root,resolve(dirname(absolute),i)).replaceAll('\\','/'):i);
    const cross=resolved.filter(i=>feature&&i.startsWith('src/features/')&&i.split('/')[2]!==feature);
    const client=resolved.filter(i=>/^src\/(?:services|product)\//.test(i));
    const store=resolved.filter(i=>i.startsWith('src/stores/'));
    const role=test?'TEST':path==='src/app/App.tsx'?'APP_COMPOSITION':/Controller|\/use[A-Z]/.test(path)?'FEATURE_CONTROLLER':path.endsWith('.tsx')?'PAGE_CONTAINER':/src\/(?:stores|services|product|types)\//.test(path)?'AUTHORITY_OR_CONTRACT':'FEATURE_MODEL';
    rows.push({path,symbol:symbols.join(', ')||'module',current_role:role,target_role:role,feature_owner:feature||path.split('/')[1],state_owner:store.length?store:counts.useState?'module local state':'none',effect_owner:counts.useEffect||counts.useLayoutEffect?'module lifecycle':'none',route_dependency:resolved.filter(i=>i.includes('/routes/')),client_dependency:client,store_dependency:store,cross_feature_dependencies:cross,prop_depth:{max_property_chain:propDepth,note:'AST proxy, actual seam prop handoff reviewed separately'},local_state_count:counts.useState,effect_count:counts.useEffect+counts.useLayoutEffect,decision:test?'KEEP':/Advanced|Workspace|WorkflowLab/.test(path)?'KEEP_ADVANCED':/legacy|resume/i.test(path)?'KEEP_COMPAT':'DEFER',replacement:null,risk:test?'Existing regression evidence.':'Retain behavior; no migration without reviewed authority boundary.',tests:[],test,lines:source.trimEnd().split('\n').length,components:symbols.filter(s=>/^[A-Z]/.test(s)).length,hooks:symbols.filter(s=>/^use[A-Z]/.test(s)).length,imports,resolvedImports:resolved,counts,deep_relative_imports:imports.filter(i=>i.startsWith('../../')).length,deep_feature_imports:resolved.filter(i=>/^src\/features\/[^/]+\/[^/]+\//.test(i)).length});
  }
  const production=rows.filter(r=>!r.test);
  const sum=k=>production.reduce((a,r)=>a+(r.counts[k]||0),0);
  return {definitions:{large_tsx:'>=500 lines',god_candidate:'>=500 TSX lines and >=8 local state or >=5 effects; review candidate only',prop_depth:'Property-chain proxy; not asserted as true component drilling depth'},rows,metrics:{ts:production.filter(r=>r.path.endsWith('.ts')).length,tsx:production.filter(r=>r.path.endsWith('.tsx')).length,tests:rows.length-production.length,components:production.reduce((a,r)=>a+r.components,0),hooks:production.reduce((a,r)=>a+r.hooks,0),contexts:sum('contexts'),stores:production.filter(r=>r.path.startsWith('src/stores/')).length,clients:production.filter(r=>r.path.startsWith('src/services/')||r.path.startsWith('src/product/')).length,largeTsx:production.filter(r=>r.path.endsWith('.tsx')&&r.lines>=500).length,godCandidates:production.filter(r=>r.path.endsWith('.tsx')&&r.lines>=500&&(r.local_state_count>=8||r.effect_count>=5)).length,useState:sum('useState'),useEffect:sum('useEffect')+sum('useLayoutEffect'),useMemo:sum('useMemo'),useCallback:sum('useCallback'),localStorage:sum('localStorage'),sessionStorage:sum('sessionStorage'),listeners:sum('listeners'),timers:sum('timers'),subscriptions:sum('subscriptions'),crossFeature:production.reduce((a,r)=>a+r.cross_feature_dependencies.length,0),deepFeature:production.reduce((a,r)=>a+r.deep_feature_imports,0),deepRelative:production.reduce((a,r)=>a+r.deep_relative_imports,0)},top20Tsx:production.filter(r=>r.path.endsWith('.tsx')).sort((a,b)=>b.lines-a.lines).slice(0,20).map(r=>({path:r.path,lines:r.lines})),top20Controllers:production.filter(r=>r.current_role==='FEATURE_CONTROLLER').sort((a,b)=>b.lines-a.lines).slice(0,20).map(r=>({path:r.path,lines:r.lines}))};
}
