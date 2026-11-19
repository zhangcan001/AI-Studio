import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const AUDIT70_COMPLETION_BASE = "7a2cee65ae793204e9c7d04677b69852d1ff67eb";
export const AUDIT70_COMPLETION_PROOF = "docs/architecture/audit70-completion-successor.json";
export const AUDIT70_COMPLETION_CHANGED = [
  "scripts/audit70-recovery-successor-guard.mjs",
  "scripts/boundary-validation-cache.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/audit70-recovery.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/convenience.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/library-hidden-prompts.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/production-safety.test.mjs",
  "scripts/testing/queue-sync-import-paging.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/run-recovery-polish.test.mjs",
  "scripts/testing/storyboard-feedback.test.mjs",
  "scripts/testing/storyboard-integrity.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "scripts/testing/usage-polish.test.mjs",
  "src-tauri/src/application/artifact_service.rs",
  "src-tauri/src/application/asset_import_service.rs",
  "src-tauri/src/application/asset_query_service.rs",
  "src-tauri/src/application/dev027_e2e.rs",
  "src-tauri/src/application/generation_input_preparer.rs",
  "src-tauri/src/application/generation_service.rs",
  "src-tauri/src/application/media_probe.rs",
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/application/product/creation_facade/submission.rs",
  "src-tauri/src/application/product/run_facade/detail.rs",
  "src-tauri/src/application/product/selected_export.rs",
  "src-tauri/src/application/product/video_inputs.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/application/shot_batch_service.rs",
  "src-tauri/src/application/shot_service.rs",
  "src-tauri/src/application/shot_service/creation.rs",
  "src-tauri/src/application/shot_video_input_service.rs",
  "src-tauri/src/application/task_recovery_service.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/infrastructure/database/repositories/generation_definition.rs",
  "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/dev052_runtime_integration.rs",
  "src-tauri/tests/dev055_live_comfy.rs",
  "src-tauri/tests/support/creation_context_contract.rs",
  "src-tauri/tests/support/creation_submission_contract.rs",
  "src-tauri/tests/support/minimax_video_input_regressions.rs",
  "src-tauri/tests/support/product_facade_contract.rs",
  "src-tauri/tests/support/run_workspace_contract.rs",
  "src/app/FeatureLifecycle.test.tsx",
  "src/features/create/CreateController.ts",
  "src/features/create/CreatePage.test.tsx",
  "src/features/create/CreateResults.tsx",
  "src/features/create/CreateVideoWorkspace.test.tsx",
  "src/features/create/CreateVideoWorkspace.tsx",
  "src/features/create/ShotReviewDialog.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/createModel.ts",
  "src/features/create/shotPromptParser.test.ts",
  "src/features/create/shotPromptParser.ts",
  "src/features/create/usePersistentVideoInputs.test.tsx",
  "src/features/create/usePersistentVideoInputs.ts",
  "src/features/projects/ProjectWorkspace.test.tsx",
  "src/features/projects/ProjectWorkspace.tsx",
  "src/features/runs/RunDetail.tsx",
  "src/features/runs/RunsPage.tsx",
  "src/features/runs/RunsPhase6C.test.tsx",
  "src/features/runs/runsModel.ts",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "src/features/shots/FolderBatchImportPanel.tsx",
  "src/features/shots/ShotBulkConfigPanel.phase5b.test.tsx",
  "src/features/shots/ShotBulkConfigPanel.tsx",
  "src/features/shots/StoryboardBatchDialog.tsx",
  "src/features/shots/useFolderBatchImportController.ts",
  "src/features/studio/ProductionQueuePanel.phase5b.test.tsx",
  "src/features/studio/ProductionQueuePanel.tsx",
  "src/product/client.test.ts",
  "src/product/errors.ts",
  "src/product/types.ts",
  "src/types/folderBatchImport.ts"
].sort();
export const AUDIT70_COMPLETION_ADDED = [
  "scripts/audit70-completion-successor-guard.mjs",
  "scripts/testing/audit70-completion.test.mjs"
].sort();
export const AUDIT70_COMPLETION_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "explicitSelectionAndReview": true,
  "boundedWorkbenchPaging": true,
  "noImplicitStart": true
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const git = (root, args) => sessionGit(root, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const factsCache = new Map();
function facts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!factsCache.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(p => base + ":" + p + "\n").join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(), blobIds = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, immutableBlobBytes(header[1], bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function audit70CompletionScopeSince(root, base) {
  const tip = facts(root, AUDIT70_COMPLETION_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...AUDIT70_COMPLETION_CHANGED, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF,
  ])].sort();
}
export function audit70CompletionFixtureFiles(root) {
  return [...facts(root, AUDIT70_COMPLETION_BASE).paths, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, AUDIT70_COMPLETION_BASE);
  const fail = id => violations.push("audit70-completion-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(AUDIT70_COMPLETION_PROOF));
    const scope = [...AUDIT70_COMPLETION_CHANGED, ...AUDIT70_COMPLETION_ADDED].sort();
    if (canonical(live(AUDIT70_COMPLETION_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "AUDIT70_COMPLETION"
        || proof.parentHead !== AUDIT70_COMPLETION_BASE || proof.planningParentHead !== AUDIT70_COMPLETION_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(AUDIT70_COMPLETION_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", AUDIT70_COMPLETION_BASE]).trim() !== AUDIT70_COMPLETION_BASE) fail("ancestor");
    const allowed = [...scope, AUDIT70_COMPLETION_PROOF].sort();
    const expected = [...parent.paths, ...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of AUDIT70_COMPLETION_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of AUDIT70_COMPLETION_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: AUDIT70_COMPLETION_CHANGED,
    addedPaths: [...AUDIT70_COMPLETION_ADDED, AUDIT70_COMPLETION_PROOF],
    afterHashes: Object.fromEntries([...AUDIT70_COMPLETION_CHANGED, ...AUDIT70_COMPLETION_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: sessionMemoRead(workspace, p => AUDIT70_COMPLETION_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function audit70CompletionParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "audit70CompletionParentReader", undefined,
    () => validate(root, undefined), AUDIT70_COMPLETION_PROOF);
}
