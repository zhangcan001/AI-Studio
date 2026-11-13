import { AUDIT_REPAIR_CHANGED, AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF } from "../audit-repair-successor-guard.mjs";
import {USABILITY_COMPLETION_CHANGED, USABILITY_COMPLETION_ADDED, USABILITY_COMPLETION_PROOF} from '../usability-completion-successor-guard.mjs';
import {STORYBOARD_USAGE_CHANGED, STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF} from '../storyboard-usage-successor-guard.mjs';
import {REFERENCE_AUTHORITY_CHANGED, REFERENCE_AUTHORITY_ADDED, REFERENCE_AUTHORITY_PROOF} from '../reference-authority-successor-guard.mjs';
import {INPUT_REFRESH_SAFETY_CHANGED, INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF} from '../input-refresh-safety-successor-guard.mjs';
// @vitest-environment node
import { expect, it } from 'vitest';
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import {
  pikaFlexReferenceParentReader,
  PIKA_FLEX_REFERENCE_BASE,
  PIKA_FLEX_REFERENCE_PROOF,
  PIKA_FLEX_REFERENCE_CHANGED,
  PIKA_FLEX_REFERENCE_ADDED,
} from '../pika-flex-reference-successor-guard.mjs';
import { createPackagePreviewParentReader, CREATE_PACKAGE_PREVIEW_PROOF } from '../create-package-preview-successor-guard.mjs';
import { uiPhase7BR1MetadataParentReader } from '../ui-phase7b-r1-metadata-successor-guard.mjs';
import { CREATE_RERUN_SAFETY_CHANGED, CREATE_RERUN_SAFETY_ADDED, CREATE_RERUN_SAFETY_PROOF } from '../create-rerun-safety-successor-guard.mjs';

const source = 'src-tauri/src/application/builtin_pika_flex_reference.rs';
const runtime = 'src-tauri/runtime_packages/minimax_h3_fl2va_i2v_quality_2_2_0/recipe.yaml';

it('verifies the additive Flex scope and projects the exact parent checkpoint', () => {
  const result = pikaFlexReferenceParentReader('.');
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...PIKA_FLEX_REFERENCE_CHANGED, ...CREATE_RERUN_SAFETY_CHANGED, ...INPUT_REFRESH_SAFETY_CHANGED, ...REFERENCE_AUTHORITY_CHANGED, ...STORYBOARD_USAGE_CHANGED, ...USABILITY_COMPLETION_CHANGED, ...AUDIT_REPAIR_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...PIKA_FLEX_REFERENCE_ADDED, PIKA_FLEX_REFERENCE_PROOF, ...CREATE_RERUN_SAFETY_ADDED, CREATE_RERUN_SAFETY_PROOF, ...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF, ...REFERENCE_AUTHORITY_ADDED, REFERENCE_AUTHORITY_PROOF, ...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF, ...USABILITY_COMPLETION_ADDED, USABILITY_COMPLETION_PROOF, ...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF])].sort());
  for (const p of PIKA_FLEX_REFERENCE_CHANGED) {
    expect(result.read(p)).toBe(execFileSync('git', ['show', `${PIKA_FLEX_REFERENCE_BASE}:${p}`], { encoding: 'utf8' }).replaceAll('\r\n', '\n'));
  }
  expect(readFileSync(CREATE_PACKAGE_PREVIEW_PROOF)).toEqual(execFileSync('git', ['show', `${PIKA_FLEX_REFERENCE_BASE}:${CREATE_PACKAGE_PREVIEW_PROOF}`]));
  expect(createPackagePreviewParentReader('.').violations).toEqual([]);
  expect(uiPhase7BR1MetadataParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects forged identity, contract and hash evidence, graph changes and Queue drift', () => {
  const proof = JSON.parse(readFileSync(PIKA_FLEX_REFERENCE_PROOF, 'utf8'));
  for (const override of [
    { [PIKA_FLEX_REFERENCE_PROOF]: JSON.stringify({ ...proof, parentHead: 'forged' }) },
    { [PIKA_FLEX_REFERENCE_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, productionQueueOnlyExecution: false } }) },
    { [PIKA_FLEX_REFERENCE_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [source]: { ...proof.files[source], afterHash: '0'.repeat(64) } } }) },
    { [source]: readFileSync(source, 'utf8').replace('0.972973', '0.7') },
    { [CREATE_PACKAGE_PREVIEW_PROOF]: readFileSync(CREATE_PACKAGE_PREVIEW_PROOF, 'utf8') + '\nforged' },
    { 'src-tauri/src/application/production_queue_service.rs': 'unapproved queue change' },
  ]) expect(pikaFlexReferenceParentReader('.', override).violations.length).toBeGreaterThan(0);
  expect(pikaFlexReferenceParentReader('.').violations).toEqual([]);
}, 120000);

it('rejects live graph, proof, raw runtime byte and illegal-file drift after cache warm-up', () => {
  const root = mkdtempSync(join(tmpdir(), 'ai-studio-pika-flex-proof-'));
  try {
    const paths = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { encoding: 'utf8' }).trim().split(/\r?\n/).filter(p => p === 'vite.config.ts' || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p));
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync('git', ['init', '-q', root]);
    const objects = resolve(execFileSync('git', ['rev-parse', '--git-path', 'objects'], { encoding: 'utf8' }).trim());
    writeFileSync(join(root, '.git/objects/info/alternates'), objects.replaceAll('\\', '/') + '\n');
    const validate = () => uiPhase7BR1MetadataParentReader(root).violations;
    expect(validate()).toEqual([]);
    const graph = join(root, source), graphBytes = readFileSync(graph);
    try { writeFileSync(graph, Buffer.concat([graphBytes, Buffer.from('\n// drift')])); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(graph, graphBytes); }
    const proof = join(root, PIKA_FLEX_REFERENCE_PROOF), proofBytes = readFileSync(proof);
    try { rmSync(proof); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(proof, proofBytes); }
    const file = join(root, runtime), bytes = readFileSync(file);
    const text = bytes.toString('utf8');
    const otherEndings = text.includes('\r\n') ? text.replaceAll('\r\n', '\n') : text.replaceAll('\n', '\r\n');
    try { writeFileSync(file, otherEndings); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(file, bytes); }
    const illegal = join(root, 'src/pika-flex-unapproved.ts');
    try { writeFileSync(illegal, 'unapproved'); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegal, { force: true }); }
    expect(validate()).toEqual([]);
  } finally {
    if (!resolve(root).startsWith(join(resolve(tmpdir()), 'ai-studio-pika-flex-proof-'))) throw Error('Unsafe fixture cleanup');
    rmSync(root, { recursive: true, force: true });
  }
}, 120000);
