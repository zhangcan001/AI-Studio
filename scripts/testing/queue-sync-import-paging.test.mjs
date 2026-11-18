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
  expect(result.changedPaths).toEqual(QUEUE_SYNC_IMPORT_PAGING_CHANGED);
  expect(result.addedPaths).toEqual([...QUEUE_SYNC_IMPORT_PAGING_ADDED, QUEUE_SYNC_IMPORT_PAGING_PROOF]);
  const parent = path => execFileSync("git", ["show", `${QUEUE_SYNC_IMPORT_PAGING_BASE}:${path}`]);
  for (const path of result.changedPaths) {
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
