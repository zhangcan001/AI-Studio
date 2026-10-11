// @vitest-environment node
import { expect, it, vi } from 'vitest';
// @ts-expect-error Node-only immutable object fixture.
import { execFileSync } from 'node:child_process';
// @ts-expect-error Node-only owned fixture.
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
// @ts-expect-error Node-only owned fixture.
import { join, resolve } from 'node:path';
// @ts-expect-error Node-only fixture isolation from desktop workspace watchers.
import { tmpdir } from 'node:os';
// @ts-expect-error Node-only proof cache.
import { immutableGit } from '../boundary-validation-cache.mjs';

vi.mock('node:child_process', async importOriginal => {
  const original = await importOriginal<any>();
  return { ...original, execFileSync: vi.fn(original.execFileSync) };
});

function fixture(action: (root: string, git: (args: string[], input?: string) => any) => void) {
  const workspace = resolve(tmpdir()), root = mkdtempSync(join(workspace, '.codex-immutable-cache-'));
  const git = (args: string[], input?: string) => execFileSync('git', args, { cwd: root, input, encoding: 'utf8' });
  try {
    git(['init', '--quiet']); git(['config', 'user.name', 'Owned test']); git(['config', 'user.email', 'owned@example.invalid']);
    action(root, git);
  } finally {
    if ((!root.startsWith(workspace + '/') && !root.startsWith(workspace + '\\')) || !root.includes('.codex-immutable-cache-')) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  }
}

it('reads identical immutable blobs only once across distinct historical commits while preserving exact batch output', () => fixture((root, git) => {
  writeFileSync(join(root, 'shared.txt'), 'shared immutable bytes\n'.repeat(10000));
  writeFileSync(join(root, 'changed.txt'), 'before\n');
  git(['add', '.']); git(['commit', '--quiet', '-m', 'one']); const before = git(['rev-parse', 'HEAD']).trim();
  writeFileSync(join(root, 'changed.txt'), 'after\n'); git(['add', '.']); git(['commit', '--quiet', '-m', 'two']); const after = git(['rev-parse', 'HEAD']).trim();
  const shared = git(['rev-parse', `${before}:shared.txt`]).trim();
  const input = (sha: string) => `${sha}:shared.txt\n${sha}:changed.txt\n${sha}:shared.txt\n`;
  const expected = [before, after].map(sha => execFileSync('git', ['--no-replace-objects', 'cat-file', '--batch'], { cwd: root, input: input(sha) }));
  vi.mocked(execFileSync).mockClear();
  [before, after].forEach((sha, i) => expect(immutableGit('git', ['cat-file', '--batch'], { cwd: root, input: input(sha) })).toEqual(expected[i]));
  const calls = vi.mocked(execFileSync).mock.calls;
  const fetched = calls.flatMap((args: any, i: number) => args[1].includes('--batch')
    ? [String(vi.mocked(execFileSync).mock.results[i].value)] : []).join('');
  expect(fetched.split(`${shared} blob `).length - 1).toBe(1);
}), 90000);

it('does not expose mutable cached buffers, follow replace refs, or hide missing immutable evidence', () => fixture((root, git) => {
  writeFileSync(join(root, 'a.txt'), 'original\n'); git(['add', '.']); git(['commit', '--quiet', '-m', 'one']);
  const sha = git(['rev-parse', 'HEAD']).trim(), input = `${sha}:a.txt\n`;
  const options = { cwd: root, input };
  const first = immutableGit('git', ['cat-file', '--batch'], options), expected = first.toString();
  first.fill(0); expect(immutableGit('git', ['cat-file', '--batch'], options).toString()).toBe(expected);
  const original = git(['rev-parse', `${sha}:a.txt`]).trim(), replacement = git(['hash-object', '-w', '--stdin'], 'replacement\n').trim();
  git(['replace', original, replacement]);
  expect(immutableGit('git', ['cat-file', '--batch'], { cwd: root, input: `${sha}:a.txt\n${sha}:a.txt\n` }).toString()).toBe(expected + expected);
  const missing = `${sha}:absent.txt\n`;
  expect(immutableGit('git', ['cat-file', '--batch'], { cwd: root, input: missing }).toString()).toBe(`${sha}:absent.txt missing\n`);
  expect(immutableGit('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()).toBe(sha);
}), 90000);

it('resolves an immutable tree once across distinct path batches without repeating commit:path queries', () => fixture((root, git) => {
  for (const name of ['a.txt', 'space name.txt', '中文.txt']) writeFileSync(join(root, name), name + '\n');
  git(['add', '.']); git(['commit', '--quiet', '-m', 'tree lookup']);
  const sha = git(['rev-parse', 'HEAD']).trim();
  vi.mocked(execFileSync).mockClear();
  for (const name of ['a.txt', 'space name.txt', '中文.txt', 'a.txt']) {
    const input = `${sha}:${name}\n`;
    expect(immutableGit('git', ['cat-file', '--batch'], { cwd: root, input }).toString()).toContain(name + '\n');
  }
  const calls = vi.mocked(execFileSync).mock.calls;
  expect(calls.filter((args: any) => args[1].includes('ls-tree'))).toHaveLength(1);
  expect(calls.some((args: any) => args[1].includes('--batch-check'))).toBe(false);
}), 90000);

it('rejects corrupted Git blob bytes before cache insertion and succeeds after restoring the real provider', async () => {
  const actual = await vi.importActual<any>('node:child_process');
  fixture((root, git) => {
    writeFileSync(join(root, 'a.txt'), 'verified bytes\n'); git(['add', '.']); git(['commit', '--quiet', '-m', 'one']);
    const sha = git(['rev-parse', 'HEAD']).trim(), options = { cwd: root, input: `${sha}:a.txt\n` };
    try {
      vi.mocked(execFileSync).mockImplementation((command: any, args: any, opts: any) => {
        const result = actual.execFileSync(command, args, opts);
        if (args.includes('--batch')) {
          const damaged = result.constructor.from(result); damaged[damaged.length - 2] ^= 1; return damaged;
        }
        return result;
      });
      expect(() => immutableGit('git', ['cat-file', '--batch'], options)).toThrow('Immutable Git blob bytes mismatch');
    } finally { vi.mocked(execFileSync).mockImplementation(actual.execFileSync); }
    expect(immutableGit('git', ['cat-file', '--batch'], options).toString()).toContain('verified bytes\n');
  });
}, 90000);

it('preserves significant path whitespace and the exact output limit instead of normalizing batch input', () => fixture((root, git) => {
  const regular = git(['hash-object', '-w', '--stdin'], 'regular bytes\n').trim();
  const trailing = git(['hash-object', '-w', '--stdin'], 'trailing-space bytes\n').trim();
  // Git permits this tree path even though Windows cannot check it out.
  const tree = git(['mktree'], `100644 blob ${regular}\tname\n100644 blob ${trailing}\tname \n`).trim();
  const sha = git(['commit-tree', tree, '-m', 'significant tree paths']).trim();
  const input = `${sha}:name\n${sha}:name \n`;
  const expected = execFileSync('git', ['--no-replace-objects', 'cat-file', '--batch'], { cwd: root, input });
  expect(immutableGit('git', ['cat-file', '--batch'], { cwd: root, input })).toEqual(expected);
  expect(() => immutableGit('git', ['cat-file', '--batch'], { cwd: root, input, maxBuffer: 1 })).toThrow();
}), 90000);

it('rejects malformed immutable tree evidence before insertion and accepts the restored provider', async () => {
  const actual = await vi.importActual<any>('node:child_process');
  fixture((root, git) => {
    writeFileSync(join(root, 'a.txt'), 'tree evidence\n'); git(['add', '.']); git(['commit', '--quiet', '-m', 'tree evidence']);
    const sha = git(['rev-parse', 'HEAD']).trim(), options = { cwd: root, input: `${sha}:a.txt\n` };
    try {
      vi.mocked(execFileSync).mockImplementation((command: any, args: any, opts: any) => {
        const value = actual.execFileSync(command, args, opts);
        return args.includes('ls-tree') ? value.subarray(0, value.length - 1) : value;
      });
      expect(() => immutableGit('git', ['cat-file', '--batch'], options)).toThrow('Immutable Git tree boundary mismatch');
    } finally { vi.mocked(execFileSync).mockImplementation(actual.execFileSync); }
    expect(immutableGit('git', ['cat-file', '--batch'], options).toString()).toContain('tree evidence\n');
  });
}, 90000);
