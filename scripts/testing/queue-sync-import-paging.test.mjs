import { AUDIT70_RECOVERY_CHANGED, AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF } from "../audit70-recovery-successor-guard.mjs";
import { AUDIT70_COMPLETION_CHANGED, AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF } from "../audit70-completion-successor-guard.mjs";
// @vitest-environment node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import {
  QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_BASE,
  QUEUE_SYNC_IMPORT_PAGING_CHANGED, QUEUE_SYNC_IMPORT_PAGING_PROOF,
  queueSyncImportPagingParentReader,
} from "../queue-sync-import-paging-successor-guard.mjs";
import { LIBRARY_HIDDEN_PROMPTS_PROOF } from "../library-hidden-prompts-successor-guard.mjs";

it("projects exact parent bytes and preserves its frozen proof", () => {
  const result = queueSyncImportPagingParentReader(".");
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...QUEUE_SYNC_IMPORT_PAGING_CHANGED, ...AUDIT70_RECOVERY_CHANGED, ...AUDIT70_COMPLETION_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF, ...AUDIT70_RECOVERY_ADDED, AUDIT70_RECOVERY_PROOF, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF])].sort());
  const parent = path => execFileSync("git", ["show", `${QUEUE_SYNC_IMPORT_PAGING_BASE}:${path}`]);
  for (const path of QUEUE_SYNC_IMPORT_PAGING_CHANGED) {
    expect(result.read(path)).toBe(parent(path).toString().replaceAll("\r\n", "\n"));
  }
  expect(readFileSync(LIBRARY_HIDDEN_PROMPTS_PROOF)).toEqual(parent(LIBRARY_HIDDEN_PROMPTS_PROOF));
}, 120000);

it("rejects a forged checkpoint, modified source, and unapproved files", () => {
  const path = "src-tauri/src/commands/task.rs";
  for (const override of [
    { [QUEUE_SYNC_IMPORT_PAGING_PROOF]: readFileSync(QUEUE_SYNC_IMPORT_PAGING_PROOF, "utf8").replace(QUEUE_SYNC_IMPORT_PAGING_BASE, "forged") },
    { [path]: readFileSync(path, "utf8") + "\n// unapproved" },
    { "src-tauri/migrations/999_unapproved.sql": "SELECT 1;" },
  ]) {
    expect(queueSyncImportPagingParentReader(".", override).violations.length).toBeGreaterThan(0);
  }
}, 120000);
