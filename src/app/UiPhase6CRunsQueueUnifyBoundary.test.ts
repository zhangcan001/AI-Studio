// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  uiPhase6CParentReader,
  PHASE6C_MANIFEST,
  PHASE6C_PARENT,
  PHASE6C_INVARIANTS,
  PHASE6C_DATABASE,
} from "../../scripts/ui-phase6c-runs-queue-unify-successor-guard.mjs";
import { uiPhase6BParentReader } from "../../scripts/ui-phase6b-library-shot-bind-successor-guard.mjs";

it("wires honest Runs/Queue RunRef projection without mixed cursor or new persistence", () => {
  const model = readFileSync("src/features/runs/runsModel.ts", "utf8");
  expect(model).toContain("coverageMessage");
  expect(model).toContain("sourceLabels");
  const page = readFileSync("src/features/runs/RunsPage.tsx", "utf8");
  expect(page).toContain("coverageMessage");
  expect(page).not.toContain("nextPage");
  const workspace = readFileSync("src-tauri/src/application/product/run_facade/workspace.rs", "utf8");
  expect(workspace).toContain("RUN_CURSOR_UNSUPPORTED");
  expect(workspace).toContain("RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES");
}, 90000);

it("accepts scoped Phase6C successor and preserves Phase6B proofs", () => {
  const reader = uiPhase6CParentReader(".");
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE6C_MANIFEST, "utf8"));
  expect(proof.parentHead).toBe(PHASE6C_PARENT);
  expect(proof.parentHead).toBe("c05314c11b32183f8ec73527645348c2f249a8c2");
  expect(PHASE6C_INVARIANTS.honestRecentCoverageNoMixedCursor).toBe(true);
  expect(PHASE6C_INVARIANTS.schemaChanged).toBe(false);
  expect(PHASE6C_INVARIANTS.noNewRunQueuePersistentModel).toBe(true);
  expect(PHASE6C_DATABASE.migrationAfter).toBe(44);
  expect(uiPhase6BParentReader(".").violations).toEqual([]);
}, 90000);
