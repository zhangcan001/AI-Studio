import { AUDIT70_CI_INTEGRATION_CHANGED, AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF } from "../audit70-ci-integration-successor-guard.mjs";
import { AUDIT70_CI_FIXTURES_CHANGED, AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF } from "../audit70-ci-fixtures-successor-guard.mjs";
import { AUDIT70_CI_VALIDATION_CHANGED, AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF } from "../audit70-ci-validation-successor-guard.mjs";
// @vitest-environment node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { runOverrideProbeSession } from "../boundary-validation-cache.mjs";
import {
  AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_BASE,
  AUDIT70_CI_REPAIR_CHANGED, AUDIT70_CI_REPAIR_PROOF,
  audit70CiRepairParentReader,
} from "../audit70-ci-repair-successor-guard.mjs";
import { AUDIT70_COMPLETION_PROOF } from "../audit70-completion-successor-guard.mjs";

it("projects exact parent bytes and preserves its frozen proof", () => {
  const result = audit70CiRepairParentReader(".");
  expect(result.violations).toEqual([]);
  expect(result.changedPaths).toEqual([...new Set([...AUDIT70_CI_REPAIR_CHANGED, ...AUDIT70_CI_VALIDATION_CHANGED, ...AUDIT70_CI_FIXTURES_CHANGED, ...AUDIT70_CI_INTEGRATION_CHANGED])].sort());
  expect(result.addedPaths).toEqual([...new Set([...AUDIT70_CI_REPAIR_ADDED, AUDIT70_CI_REPAIR_PROOF, ...AUDIT70_CI_VALIDATION_ADDED, AUDIT70_CI_VALIDATION_PROOF, ...AUDIT70_CI_FIXTURES_ADDED, AUDIT70_CI_FIXTURES_PROOF, ...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF])].sort());
  const parent = path => execFileSync("git", ["show", `${AUDIT70_CI_REPAIR_BASE}:${path}`]);
  for (const path of AUDIT70_CI_REPAIR_CHANGED) {
    expect(result.read(path)).toBe(parent(path).toString().replaceAll("\r\n", "\n"));
  }
  expect(readFileSync(AUDIT70_COMPLETION_PROOF)).toEqual(parent(AUDIT70_COMPLETION_PROOF));
}, 120000);

it("rejects a forged checkpoint, modified source, and unapproved files", () => {
  const path = "src-tauri/src/application/production_queue_service.rs";
  runOverrideProbeSession(".", () => {
    for (const override of [
      { [AUDIT70_CI_REPAIR_PROOF]: readFileSync(AUDIT70_CI_REPAIR_PROOF, "utf8").replace(AUDIT70_CI_REPAIR_BASE, "forged") },
      { [path]: readFileSync(path, "utf8") + "\n// unapproved" },
      { "src-tauri/migrations/999_unapproved.sql": "SELECT 1;" },
    ]) {
      expect(audit70CiRepairParentReader(".", override).violations.length).toBeGreaterThan(0);
    }
  });
}, 120000);

it("checks project ownership before approving an existing readable artifact", () => {
  const source = readFileSync("src-tauri/src/application/artifact_service.rs", "utf8");
  const review = source.split("pub async fn submit_review(")[1].split("let original = review.clone()")[0];
  expect(review.indexOf(".find_artifact(project_id, &artifact_id)")).toBeLessThan(review.indexOf(".resolve_artifact_path(artifact_id.as_str())"));
  expect(review).toContain("decision == ArtifactReviewDecision::Approved");
  expect(review).not.toContain("resolve_artifact_path(project_id,");
});
