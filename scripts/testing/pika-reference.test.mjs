// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { pikaReferenceParentReader, PIKA_REFERENCE_BASE, PIKA_REFERENCE_PROOF, PIKA_REFERENCE_CHANGED, PIKA_REFERENCE_ADDED, PIKA_REFERENCE_CONTRACTS } from '../pika-reference-successor-guard.mjs';
import { ciConsolidateFixParentReader } from '../ci-consolidate-fix-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';
const source = 'src-tauri/src/application/builtin_pika_reference.rs';
const oldProof = 'docs/architecture/ci-consolidate-fix-successor.json';

it('validates the Pika checkpoint and projects exact parent bytes without rewriting frozen proofs', () => {
  const checked = pikaReferenceParentReader('.');
  expect(checked.violations).toEqual([]);
  for (const path of PIKA_REFERENCE_CHANGED) {
    expect(checked.read(path)).toBe(execFileSync('git', ['show', `${PIKA_REFERENCE_BASE}:${path}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(oldProof)).toEqual(execFileSync('git', ['show', `${PIKA_REFERENCE_BASE}:${oldProof}`]));
  expect(ciConsolidateFixParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged proof identities, contracts, hashes and undeclared Queue or Runtime edits', () => {
  const proof = JSON.parse(readFileSync(PIKA_REFERENCE_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [PIKA_REFERENCE_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(PIKA_REFERENCE_CONTRACTS).map(([key, value]) => ({ [PIKA_REFERENCE_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [PIKA_REFERENCE_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    { [PIKA_REFERENCE_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], afterHash: '0'.repeat(64) } } }) },
    ...[...PIKA_REFERENCE_CHANGED, ...PIKA_REFERENCE_ADDED, oldProof].map(path => ({ [path]: readFileSync(path, 'utf8') + '\nforged' })),
    { 'src-tauri/src/application/production_queue_service.rs': 'changed queue' },
    { 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml': 'changed runtime' },
    { 'src/pika-unapproved.ts': 'illegal addition' },
  ]) expect(pikaReferenceParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(pikaReferenceParentReader('.').violations).toEqual([]);
}, 120000);

it('invalidates warm caches on live mutation, deletion, illegal files and raw Runtime byte drift; restoration passes', () => {
  const root = mkdtempSync(join(tmpdir(), 'pika-reference-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(path => path === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(path));
    for (const path of paths) { mkdirSync(dirname(join(root, path)), { recursive: true }); copyFileSync(path, join(root, path)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const path of [PIKA_REFERENCE_PROOF, oldProof, source, 'src-tauri/src/application/production_queue_service.rs', 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml']) {
      const file = join(root, path), bytes = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([bytes, Buffer.from(path === PIKA_REFERENCE_PROOF ? '\nmutation' : '\n')])); expect(validate().length, path).toBeGreaterThan(0);
        rmSync(file); expect(validate().length, path).toBeGreaterThan(0);
      } finally { writeFileSync(file, bytes); }
      expect(validate()).toEqual([]);
    }
    const illegal = join(root, 'src/pika-unapproved.ts');
    try { writeFileSync(illegal, 'illegal'); expect(validate().length, 'illegal file').toBeGreaterThan(0); }
    finally { rmSync(illegal, { force: true }); }
    expect(validate()).toEqual([]);
  } finally {
    if (!resolve(root).startsWith(resolve(tmpdir()) + '\\pika-reference-proof-') && !resolve(root).startsWith(resolve(tmpdir()) + '/pika-reference-proof-')) throw Error('unsafe owned fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 120000);
