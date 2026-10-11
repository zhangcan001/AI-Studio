// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  uiPhase7AParentReader,
  PHASE7A_MANIFEST,
  PHASE7A_PARENT,
  PHASE7A_INVARIANTS,
  PHASE7A_DATABASE,
} from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";
import { uiPhase6DParentReader } from "../../scripts/ui-phase6d-scale-integrity-successor-guard.mjs";

it("wires Phase7A source gate repairs without Native/GPU claims", () => {
  const pkg = readFileSync("package.json", "utf8");
  expect(pkg).toContain("test:boundary");
  const shim = readFileSync("src/types/mjs-shims.d.ts", "utf8");
  expect(shim).toContain('declare module "*.mjs"');
  expect(shim).toContain('declare module "node:fs"');
  const retirement = readFileSync("src/app/Phase7Retirement.test.tsx", "utf8");
  expect(retirement).toContain("120000");
}, 90000);

it("accepts scoped Phase7A successor and preserves Phase6D proofs", () => {
  const reader = uiPhase7AParentReader(".");
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE7A_MANIFEST, "utf8"));
  expect(proof.parentHead).toBe(PHASE7A_PARENT);
  expect(proof.parentHead).toBe("0d355a166d136e2148088f799f5a94b4c914f41e");
  expect(PHASE7A_INVARIANTS.sourceGatesRepaired).toBe(true);
  expect(PHASE7A_INVARIANTS.nativeGpuExecuted).toBe(false);
  expect(PHASE7A_INVARIANTS.schemaChanged).toBe(false);
  expect(PHASE7A_DATABASE.migrationAfter).toBe(44);
  expect(uiPhase6DParentReader(".").violations).toEqual([]);
}, 120000);
