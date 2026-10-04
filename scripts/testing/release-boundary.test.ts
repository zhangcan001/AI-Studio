import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { releaseParentReader } from '../phase14-release-guard.mjs';

const proof = JSON.parse(readFileSync('docs/architecture/phase14-release.json', 'utf8'));
const rust = 'src-tauri/src/application/project_backup_service.rs';

describe('exact release-only serialization successor', () => {
  it('verifies current bytes before exposing the immutable historical parent', () => {
    const gate = releaseParentReader('.');
    expect(gate.violations).toEqual([]);
    expect(gate.read(rust)).not.toBe(readFileSync(rust, 'utf8'));
  });

  it.each(['path', 'untouched', 'scope', 'authority', 'workflow'])('rejects %s drift', (kind) => {
    const invalid = structuredClone(proof);
    if (kind === 'path') invalid.paths[rust].afterHash = '0'.repeat(64);
    if (kind === 'untouched') invalid.backend.untouchedAggregateHash = '0'.repeat(64);
    if (kind === 'scope') invalid.paths['src-tauri/src/domain/task.rs'] = invalid.paths[rust];
    if (kind === 'authority') invalid.invariants.taskStateMachineChanged = true;
    if (kind === 'workflow') invalid.paths['.github/workflows/ci.yml'].afterHash = '0'.repeat(64);
    const gate = releaseParentReader('.', invalid);
    expect(gate.violations.length).toBeGreaterThan(0);
    expect(gate.read(rust)).toBe(readFileSync(rust, 'utf8'));
  });
});
