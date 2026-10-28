// @vitest-environment node
// Checks for this successor tip run in scripts/testing/ci-chain-consolidated.test.mjs (one shared workspace copy).
import { expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
it('ci-assert-fix checks run in the consolidated single-copy chain test', () => {
  expect(readFileSync('scripts/testing/ci-chain-consolidated.test.mjs', 'utf8')).toContain("describe('ci-assert-fix', () => {");
});
