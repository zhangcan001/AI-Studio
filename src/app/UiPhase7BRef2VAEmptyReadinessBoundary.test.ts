// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  PHASE7B_BASE,
  PHASE7B_PROOF,
  uiPhase7BParentReader,
} from "../../scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs";
import { uiPhase7AParentReader } from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";

it("preserves the frozen Phase7A history while authorizing the REF2VA native repair", () => {
  const result = uiPhase7BParentReader(".");
  expect(result.violations).toEqual([]);
  expect(uiPhase7AParentReader(".").violations).toEqual([]);
  expect(PHASE7B_BASE).toBe("6f5c3c33860749ccf25aff3d079b9982492fca37");
  expect(JSON.parse(readFileSync(PHASE7B_PROOF, "utf8")).parentHead).toBe(PHASE7B_BASE);
  const projected = result.read("src-tauri/src/application/product/h3_resolution.rs");
  const live = readFileSync("src-tauri/src/application/product/h3_resolution.rs", "utf8");
  expect(projected).toContain("if audios > 0 && images + videos == 0");
  expect(live).toContain("if images + videos == 0");
}, 120000);

it("rejects a modified repair byte rather than weakening the old proofs", () => {
  const path = "src-tauri/src/application/product/h3_resolution.rs";
  const live = readFileSync(path, "utf8");
  const mutated = live.replace("if images + videos == 0", "if audios > 0 && images + videos == 0");
  expect(mutated).not.toBe(live);
  const candidate = uiPhase7BParentReader(".", { [path]: mutated });
  expect(candidate.violations).not.toEqual([]);
  expect(candidate.violations.some((v: string) => v.includes("changed:" + path) || v.includes("minimum-media-gate"))).toBe(true);
}, 120000);
