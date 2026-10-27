// @vitest-environment node
import { expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { uiPhase7CParentReader, PHASE7C_PARENT, PHASE7C_PROOF } from "../../scripts/ui-phase7c-frontend-ci-budget-successor-guard.mjs";
import { workbenchVisualParentReader } from "../../scripts/workbench-visual-refinement-successor-guard.mjs";
import { desktopVisualParentReader } from "../../scripts/desktop-visual-refinement-successor-guard.mjs";
import { uiPhase7AParentReader } from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";

it("authorizes the frontend CI budget repair while projecting frozen historical bytes", () => {
  const reader = uiPhase7CParentReader(".");
  expect(reader.violations).toEqual([]);
  const readiness = readFileSync("src/app/ReadinessLifecycleBoundary.test.ts", "utf8");
  const tests = [...readiness.matchAll(/^[ \t]*it\s*\(/gm)];
  const budgets = [...readiness.matchAll(/\}\)?\s*,\s*90000\s*\)/g)];
  expect(tests).toHaveLength(6);
  expect(budgets.length).toBeGreaterThanOrEqual(tests.length);
  expect(readiness).not.toContain("360000");
  const ciRepair = readFileSync("src/app/UiPhase5CiRepairBoundary.test.ts", "utf8");
  expect(ciRepair).toMatch(/keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion[\s\S]*?\},\s*90000\);/);
  expect(reader.read("src/app/ReadinessLifecycleBoundary.test.ts")).toBe(
    execFileSync("git", ["show", `${PHASE7C_PARENT}:src/app/ReadinessLifecycleBoundary.test.ts`], { encoding: "utf8" }).replaceAll("\r\n", "\n"),
  );
  expect(readFileSync(PHASE7C_PROOF, "utf8")).toContain("UI_PHASE7C_FRONTEND_CI_BUDGET_SUCCESSOR");
  expect(workbenchVisualParentReader(".").violations).toEqual([]);
  expect(desktopVisualParentReader(".").violations).toEqual([]);
  expect(uiPhase7AParentReader(".").violations).toEqual([]);
}, 120000);

it("rejects forged phase metadata and does not authorize unrelated source drift", () => {
  const proof = JSON.parse(readFileSync(PHASE7C_PROOF, "utf8"));
  expect(uiPhase7CParentReader(".", { [PHASE7C_PROOF]: JSON.stringify({ ...proof, parentHead: "0".repeat(40) }) }).violations.length).toBeGreaterThan(0);
  expect(uiPhase7CParentReader(".", { "src/features/create/CreatePage.css": ":root { color: red; }" }).violations.length).toBeGreaterThan(0);
  expect(uiPhase7CParentReader(".").violations).toEqual([]);
}, 120000);
