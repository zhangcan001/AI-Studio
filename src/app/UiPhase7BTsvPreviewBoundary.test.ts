// @vitest-environment node
import { it, expect } from "vitest";
import { readFileSync } from "node:fs";
import {
  PHASE7B_TSV_BASE, PHASE7B_TSV_PROOF, uiPhase7BTsvParentReader,
} from "../../scripts/ui-phase7b-tsv-preview-successor-guard.mjs";
import { uiPhase7BParentReader } from "../../scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs";
import { uiPhase7AParentReader } from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";

it("keeps frozen Phase7A and REF2VA proof while approving TSV source validation", () => {
  const proof = JSON.parse(readFileSync(PHASE7B_TSV_PROOF, "utf8"));
  expect(proof.parentHead).toBe(PHASE7B_TSV_BASE);
  expect(proof.contracts.runtimeBytesUnchanged).toBe(true);
  expect(uiPhase7BTsvParentReader(".").violations).toEqual([]);
  expect(uiPhase7BParentReader(".").violations).toEqual([]);
  expect(uiPhase7AParentReader(".").violations).toEqual([]);
}, 120000);

it("rejects mutation of the TSV asynchronous preview gate before any product write", () => {
  const path = "src/features/shots/ShotBatchAssetBindPanel.tsx";
  const source = readFileSync(path, "utf8");
  const mutation = source.replace("await validateTsvAssets(resolveBatchBindRows", "resolveBatchBindRows");
  expect(mutation).not.toEqual(source);
  const result = uiPhase7BTsvParentReader(".", { [path]: mutation });
  expect(result.violations.some((v: string) => v.includes("changed:" + path) || v.includes("preview-guard"))).toBe(true);
}, 120000);
