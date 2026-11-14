import { USAGE_POLISH_CHANGED, USAGE_POLISH_ADDED, USAGE_POLISH_PROOF } from "../usage-polish-successor-guard.mjs";
import { CONVENIENCE_CHANGED, CONVENIENCE_ADDED, CONVENIENCE_PROOF } from "../convenience-successor-guard.mjs";
// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import {
  usageFlowParentReader, USAGE_FLOW_BASE, USAGE_FLOW_PROOF,
  USAGE_FLOW_CHANGED, USAGE_FLOW_ADDED, USAGE_FLOW_CONTRACTS,
} from '../usage-flow-successor-guard.mjs';
import { auditRepairParentReader as pikaFlexReferenceParentReader, AUDIT_REPAIR_PROOF as PIKA_FLEX_REFERENCE_PROOF } from '../audit-repair-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';

const source = 'src/features/create/StoryboardShotRows.tsx';
const runtime = 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml';
const parentBytes = p => execFileSync('git', ['show', `${USAGE_FLOW_BASE}:${p}`]);

it('verifies the exact usage-flow successor and projects the frozen parent bytes', () => {
  const result = usageFlowParentReader('.');
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...USAGE_FLOW_CHANGED, ...CONVENIENCE_CHANGED, ...USAGE_POLISH_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...USAGE_FLOW_ADDED, USAGE_FLOW_PROOF, ...CONVENIENCE_ADDED, CONVENIENCE_PROOF, ...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF])].sort());
  for (const p of USAGE_FLOW_CHANGED) expect(result.read(p)).toBe(parentBytes(p).toString().replaceAll('\r\n', '\n'));
  expect(readFileSync(PIKA_FLEX_REFERENCE_PROOF)).toEqual(parentBytes(PIKA_FLEX_REFERENCE_PROOF));
  expect(pikaFlexReferenceParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged identity, scope, contracts, hashes and unapproved Queue or Runtime edits', () => {
  const proof = JSON.parse(readFileSync(USAGE_FLOW_PROOF, 'utf8'));
  for (const override of [
    ...['checkpoint', 'parentHead', 'planningParentHead'].map(key => ({ [USAGE_FLOW_PROOF]: JSON.stringify({ ...proof, [key]: 'forged' }) })),
    ...Object.entries(USAGE_FLOW_CONTRACTS).map(([key, value]) => ({ [USAGE_FLOW_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, [key]: !value } }) })),
    { [USAGE_FLOW_PROOF]: JSON.stringify({ ...proof, files: {} }) },
    ...['beforeHash', 'afterHash'].map(key => ({ [USAGE_FLOW_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], [key]: '0'.repeat(64) } } }) })),
    ...[...USAGE_FLOW_CHANGED, ...USAGE_FLOW_ADDED, PIKA_FLEX_REFERENCE_PROOF, runtime].map(p => ({ [p]: readFileSync(p, 'utf8') + '\n// unapproved' })),
    { 'src-tauri/src/application/production_queue_service.rs': 'unapproved queue edit' },
    { 'src-tauri/migrations/999_unapproved.sql': 'SELECT 1;' },
  ]) expect(usageFlowParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(usageFlowParentReader('.').violations).toEqual([]);
}, 120000);

it('keeps live freshness through the full historical chain and rejects deletion, illegal files and raw Runtime drift', () => {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-reference-authority-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [source, USAGE_FLOW_PROOF, PIKA_FLEX_REFERENCE_PROOF]) {
      const file = join(root, p), original = readFileSync(file);
      try {
        writeFileSync(file, Buffer.concat([original, Buffer.from('\nunapproved')]));
        expect(validate().length).toBeGreaterThan(0);
        rmSync(file);
        expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(file, original); }
      expect(validate()).toEqual([]);
    }
    const file = join(root, runtime), original = readFileSync(file), text = original.toString();
    try {
      writeFileSync(file, text.includes('\r\n') ? text.replaceAll('\r\n', '\n') : text.replaceAll('\n', '\r\n'));
      expect(validate().length).toBeGreaterThan(0);
    } finally { writeFileSync(file, original); }
    expect(validate()).toEqual([]);
    const illegal = join(root, 'src/rerun-safety-unapproved.ts');
    try {
      writeFileSync(join(root, '.git/info/exclude'), 'src/rerun-safety-unapproved.ts\n');
      writeFileSync(illegal, 'unapproved');
      expect(validate().length).toBeGreaterThan(0);
    } finally { rmSync(illegal, { force: true }); }
    expect(validate()).toEqual([]);
  } finally {
    if (!resolve(root).startsWith(join(resolve(tmpdir()), 'ai-studio-reference-authority-'))) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 120000);
