// @vitest-environment node
import { it, expect } from "vitest";
import { readFileSync } from "node:fs";
import {
  PHASE7B_RECEIPT_BASE, PHASE7B_RECEIPT_PROOF, uiPhase7BTsvReceiptParentReader,
} from "../../scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs";
import { uiPhase7BTsvParentReader } from "../../scripts/ui-phase7b-tsv-preview-successor-guard.mjs";
import { uiPhase7BParentReader } from "../../scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs";
import { uiPhase7AParentReader } from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";

it("keeps frozen Phase7A/REF2VA/TSV proofs while approving receipt-eligibility source validation", () => {
  const proof = JSON.parse(readFileSync(PHASE7B_RECEIPT_PROOF, "utf8"));
  expect(proof.parentHead).toBe(PHASE7B_RECEIPT_BASE);
  expect(proof.contracts.runtimeBytesUnchanged).toBe(true);
  expect(uiPhase7BTsvReceiptParentReader(".").violations).toEqual([]);
  expect(uiPhase7BTsvParentReader(".").violations).toEqual([]);
  expect(uiPhase7BParentReader(".").violations).toEqual([]);
  expect(uiPhase7AParentReader(".").violations).toEqual([]);
}, 120000);

it("rejects dropping the receipt eligibility gate from TSV preview", () => {
  const path = "src/features/shots/ShotBatchAssetBindPanel.tsx";
  const source = readFileSync(path, "utf8");
  const mutation = source.replace("detail.asset.hasExternalImportReceipt !== true", "true");
  expect(mutation).not.toEqual(source);
  const result = uiPhase7BTsvReceiptParentReader(".", { [path]: mutation });
  expect(result.violations.some((v) => v.includes("changed:" + path) || v.includes("preview-guard"))).toBe(true);
}, 120000);
