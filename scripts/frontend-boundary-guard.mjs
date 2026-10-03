// Phase10 extends DEV-088 with scoped, non-expanding frontend boundaries.
import { frontendInventory } from './frontend-architecture-inventory.mjs';
import { read } from './style-boundary-guard.mjs';
export function frontendBoundary(root,manifest) {
  const inventory=frontendInventory(root), violations=[];
  for(const row of inventory.rows.filter(r=>!r.test)) {
    const grandfathered=manifest.importDebt[row.path]||[];
    for(const dependency of row.resolvedImports) {
      const guarded=/^src\/features\//.test(dependency)&&((row.feature_owner!==dependency.split('/')[2]&&row.path.startsWith('src/features/'))||row.path==='src/app/App.tsx');
      if(guarded&&!grandfathered.includes(dependency))violations.push(`feature-import:${row.path}:${dependency}`);
      if(dependency.startsWith('@tauri-apps/')&&!(manifest.tauriImporters[row.path]||[]).includes(dependency))violations.push(`tauri-import:${row.path}:${dependency}`);
    }
    if(row.path==='src/app/App.tsx')for(const marker of manifest.retiredRootResponsibilities)if(read(`${root}/${row.path}`).includes(marker))violations.push(`root-responsibility:${marker}`);
  }
  const allowed=new Set(manifest.seams.flatMap(s=>s.paths));
  for(const path of Object.keys(manifest.frontendSuccessor))if(!allowed.has(path))violations.push(`unscoped-successor:${path}`);
  if(manifest.rows.some(r=>['UNKNOWN','UNCLASSIFIED'].includes(r.decision)))violations.push('unclassified-matrix');
  return {violations,inventory};
}
