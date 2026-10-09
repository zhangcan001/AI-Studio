// @vitest-environment node
import { afterEach, expect, it, vi } from 'vitest';
// @ts-expect-error Node-only safety/performance fixture.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
// @ts-expect-error Node-only fixture.
import { join, resolve } from 'node:path';
// @ts-expect-error Node-only fixture.
import { tmpdir } from 'node:os';
// @ts-expect-error Node-only fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Existing JavaScript boundary API.
import { cachedBoundary, immutableGit } from '../boundary-validation-cache.mjs';
// @ts-expect-error Node-only legal successor.
import { uiPhase3CiRepairParentReader, CI_REPAIR_MANIFEST } from '../ui-phase3-ci-repair-successor-guard.mjs';

vi.mock('node:child_process', async importOriginal => {
  const actual = await importOriginal<{execFileSync: (...args: unknown[]) => unknown}>();
  return { ...actual, execFileSync: vi.fn(actual.execFileSync) };
});
const owned: string[] = [];
const temp = () => { const root = mkdtempSync(join(tmpdir(), 'ai-studio-boundary-cache-')); owned.push(root); return root; };
afterEach(() => {
  for (const root of owned.splice(0)) {
    if (!resolve(root).startsWith(resolve(tmpdir()) + '\\') && !resolve(root).startsWith(resolve(tmpdir()) + '/')) throw Error('fixture outside owned temp parent');
    if (!root.includes('ai-studio-boundary-cache-')) throw Error('unexpected fixture');
    rmSync(root, {recursive:true, force:true});
  }
  vi.mocked(execFileSync).mockClear();
});

it('reuses immutable Git IO across owned fixtures borrowing the same object store, not unrelated repositories', () => {
  const git = execFileSync('git', ['rev-parse', '--absolute-git-dir'], {encoding:'utf8'}).trim();
  const roots = [temp(), temp(), temp()];
  for (const [i, root] of roots.entries()) {
    execFileSync('git', ['init', '--quiet', root]);
    if (i < 2) writeFileSync(join(root, '.git/objects/info/alternates'), join(git, 'objects') + '\n');
  }
  vi.mocked(execFileSync).mockClear();
  const args = ['ls-tree', '-r', '--name-only', '1702f3b4b1434fc3d02c96a63346a272740b21d4', 'src/app/routes'];
  const first = immutableGit('git', args, {cwd:roots[0], encoding:'utf8'});
  expect(immutableGit('git', args, {cwd:roots[1], encoding:'utf8'})).toBe(first);
  // Count actual subprocesses, not a noisy wall-clock threshold.
  expect(execFileSync).toHaveBeenCalledTimes(1);
  expect(() => immutableGit('git', args, {cwd:roots[2], encoding:'utf8'})).toThrow();
});

it('keeps returned raw Git buffers isolated from callers', () => {
  const args = ['cat-file', '--batch'];
  const options = {input:'1702f3b4b1434fc3d02c96a63346a272740b21d4:README.md\n'};
  const first = immutableGit('git', args, options);
  const original = first.toString(); first.fill(0);
  expect(immutableGit('git', args, options).toString()).toBe(original);
});

it('never treats a hexadecimal path argument as an immutable tree when the tree is a mutable tag', () => {
  vi.mocked(execFileSync).mockClear();
  const args = ['ls-tree','-r','--name-only','v2.1.0-personal','a'.repeat(40)];
  immutableGit('git',args,{encoding:'utf8'}); immutableGit('git',args,{encoding:'utf8'});
  expect(execFileSync).toHaveBeenCalledTimes(2);
});

it('reads fresh raw bytes despite equal size/timestamps and catches proof deletion, addition and tag changes', () => {
  const root = temp(), source = join(root,'src/input.ts'), proof = join(root,'docs/proof.json'), tag = join(root,'.git/refs/tags/v2.1.0-personal');
  for (const dir of ['src','docs','.git/refs/tags','src-tauri/runtime_packages']) mkdirSync(join(root,dir),{recursive:true});
  writeFileSync(source,'alpha'); writeFileSync(proof,'{"valid":true}'); writeFileSync(tag,'expected');
  const raw = join(root,'src-tauri/runtime_packages/input.json'); writeFileSync(raw,'{}\r\n');
  let validations = 0;
  const validate = () => {
    validations++;
    const valid = existsSync(proof) && readFileSync(proof,'utf8') === '{"valid":true}' && readFileSync(source,'utf8') === 'alpha'
      && readFileSync(raw,'utf8') === '{}\r\n' && readFileSync(tag,'utf8') === 'expected' && !existsSync(join(root,'src/illegal.ts'));
    return {violations:valid ? [] : ['drift'], read:(p:string)=>readFileSync(join(root,p),'utf8')};
  };
  const reader = () => cachedBoundary(root,'owned-byte-validation',undefined,validate,'docs/proof.json');
  expect(reader().violations).toEqual([]); expect(reader().violations).toEqual([]); expect(validations).toBe(1);
  const stat = statSync(source); writeFileSync(source,'omega'); utimesSync(source,stat.atime,stat.mtime);
  expect(reader().violations).toEqual(['drift']); writeFileSync(source,'alpha'); expect(reader().violations).toEqual([]);
  for (const [path, value] of [[proof,'{"valid":false}'],[raw,'{}\n'],[tag,'wrong']] as const) {
    const bytes = readFileSync(path); writeFileSync(path,value); expect(reader().violations).toEqual(['drift']);
    writeFileSync(path,bytes); expect(reader().violations).toEqual([]);
  }
  const bytes = readFileSync(proof); rmSync(proof); expect(reader().violations).toEqual(['drift']);
  writeFileSync(proof,bytes); expect(reader().violations).toEqual([]);
  writeFileSync(join(root,'src/illegal.ts'),'unreviewed'); expect(reader().violations).toEqual(['drift']);
  rmSync(join(root,'src/illegal.ts')); expect(reader().violations).toEqual([]);
});

it('validates the new repair before projecting unchanged historical proof bytes and rejects forged authority', () => {
  const accepted = uiPhase3CiRepairParentReader('.');
  expect(accepted.violations).toEqual([]);
  const path = 'scripts/boundary-validation-cache.mjs';
  expect(accepted.read(path)).not.toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
  const forged = JSON.parse(readFileSync(CI_REPAIR_MANIFEST,'utf8'));
  forged.invariants.queueAuthorityChanged = true;
  const denied = uiPhase3CiRepairParentReader('.',forged);
  expect(denied.violations.length).toBeGreaterThan(0); expect(denied.addedPaths).toEqual([]);
  expect(denied.read(path)).toBe(readFileSync(path,'utf8').replaceAll('\r\n','\n'));
});
