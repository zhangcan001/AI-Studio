import { createHash } from "node:crypto";
import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

const REPAIR_BASE = "587633960100382e2b23df0cca0fb0b98f72ffa0";
const REPAIR_SCOPE = [
  "src-tauri/src/main.rs",
  "src-tauri/src/application/media_probe.rs",
  "scripts/audit70-ci-integration-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/audit70-ci-fixtures.test.mjs",
  "scripts/testing/audit70-ci-integration.test.mjs",
  "scripts/testing/audit70-ci-repair.test.mjs",
  "scripts/testing/audit70-ci-validation.test.mjs",
  "scripts/testing/audit70-completion.test.mjs",
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
  "src-tauri/src/application/asset_deletion_service.rs",
  "src-tauri/src/application/ports/asset_store.rs",
  "src-tauri/src/application/ports/production_orchestrator_repository.rs",
  "src-tauri/src/application/ports/workflow_benchmark_repository.rs",
  "src-tauri/src/application/product/selected_export.rs",
  "src-tauri/src/application/production_orchestrator_service.rs",
  "src-tauri/src/application/production_package_service.rs",
  "src-tauri/src/application/production_queue_service.rs",
  "src-tauri/src/application/production_start_admission_service.rs",
  "src-tauri/src/application/settings_service.rs",
  "src-tauri/src/application/task_recovery_service.rs",
  "src-tauri/src/application/workflow_benchmark_service.rs",
  "src-tauri/src/application/workflow_lifecycle_coordinator.rs",
  "src-tauri/src/commands/comfy.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/infrastructure/comfy/client.rs",
  "src-tauri/src/infrastructure/database/repositories/production_orchestrator.rs",
  "src-tauri/src/infrastructure/database/repositories/production_queue.rs",
  "src-tauri/src/infrastructure/database/repositories/workflow_benchmark.rs",
  "src-tauri/src/infrastructure/filesystem/asset_store.rs",
  "src-tauri/src/infrastructure/filesystem/workflow_file_store.rs",
  "src-tauri/src/infrastructure/settings/mod.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/dev059_production_package.rs",
  "src-tauri/tests/support/product_facade_contract.rs",
  "src-tauri/tests/support/run_workspace_contract.rs",
  "src/features/production/ProductionPackageWorkspace.tsx",
  "src/features/projects/ExternalAgentHandoffPanel.test.tsx",
  "src/features/projects/ExternalAgentHandoffPanel.tsx",
  "src/features/projects/ProjectImportDryRunWorkspace.test.tsx",
  "src/features/projects/ProjectImportDryRunWorkspace.tsx",
  "src/features/runs/RunsPage.test.tsx",
].sort();
export const PROJECT_AUDIT_REPAIR_PROOF = "docs/architecture/project-audit-repair-successor.json";
export const AUDIT70_CI_INTEGRATION_BASE = "1650cc570591f06c8811603f98c61e19488c5b1b";
export const AUDIT70_CI_INTEGRATION_PROOF = "docs/architecture/audit70-ci-integration-successor.json";
export const AUDIT70_CI_INTEGRATION_CHANGED = [
  "scripts/audit70-ci-fixtures-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/audit70-ci-fixtures.test.mjs",
  "scripts/testing/audit70-ci-repair.test.mjs",
  "scripts/testing/audit70-ci-validation.test.mjs",
  "scripts/testing/audit70-completion.test.mjs",
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
  "src-tauri/tests/dev061b_queue_recovery.rs",
  "src-tauri/tests/support/creation_context_contract.rs"
].sort();
export const AUDIT70_CI_INTEGRATION_ADDED = [
  "scripts/audit70-ci-integration-successor-guard.mjs",
  "scripts/testing/audit70-ci-integration.test.mjs"
].sort();
export const AUDIT70_CI_INTEGRATION_CONTRACTS = {
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
export function audit70CiIntegrationScopeSince(root, base) {
  const tip = facts(root, AUDIT70_CI_INTEGRATION_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...AUDIT70_CI_INTEGRATION_CHANGED, ...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF,
    ...(existsSync(join(root, PROJECT_AUDIT_REPAIR_PROOF)) ? [PROJECT_AUDIT_REPAIR_PROOF] : []),
  ])].sort();
}
export function audit70CiIntegrationFixtureFiles(root) {
  return [...facts(root, AUDIT70_CI_INTEGRATION_BASE).paths, ...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF, ...(existsSync(join(root, PROJECT_AUDIT_REPAIR_PROOF)) ? [PROJECT_AUDIT_REPAIR_PROOF] : [])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
// Authenticate the new delta before supplying the old snapshot to its guards.
function projectRepair(workspace, live, override) {
  const baseline = facts(workspace, REPAIR_BASE);
  const proof = JSON.parse(live(PROJECT_AUDIT_REPAIR_PROOF));
  const scope = Object.keys(proof.files).sort();
  if (proof.base !== REPAIR_BASE || proof.schemaVersion !== 1
      || Object.keys(proof).sort().join() !== "base,files,schemaVersion"
      || JSON.stringify(scope) !== JSON.stringify(REPAIR_SCOPE)
      || live(PROJECT_AUDIT_REPAIR_PROOF) !== JSON.stringify(proof, null, 2) + "\n") {
    throw Error("invalid project audit repair proof");
  }
  const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
    .trim().split(/\r?\n/).filter(relevant).sort();
  const expected = [...baseline.paths, PROJECT_AUDIT_REPAIR_PROOF].sort();
  if (JSON.stringify([...new Set(inventory)]) !== JSON.stringify(expected)) throw Error("repair inventory mismatch");
  const changed = baseline.paths.filter(path => live(path) !== baseline.read(path));
  if (JSON.stringify(changed) !== JSON.stringify(scope)
      || scope.some(path => !/^(src\/|src-tauri\/(src|tests)\/|scripts\/)/.test(path))
      || Object.keys(override ?? {}).some(path => relevant(path) && !expected.includes(path))) {
    throw Error("unapproved project audit repair paths");
  }
  for (const path of scope) {
    const { beforeHash, afterHash } = proof.files[path];
    if (beforeHash !== sha(baseline.read(path)) || afterHash !== sha(live(path))) {
      throw Error("project audit repair drift: " + path);
    }
  }
  const reviewed = new Set(scope);
  return { read: path => reviewed.has(path) ? baseline.read(path) : live(path), hashes: Object.fromEntries(scope.map(path => [path, proof.files[path].afterHash])) };
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, AUDIT70_CI_INTEGRATION_BASE);
  const fail = id => violations.push("audit70-ci-integration-" + id);
  const readLive = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  const addedRepair = existsSync(join(workspace, PROJECT_AUDIT_REPAIR_PROOF));
  let live = readLive;
  let repairHashes = {};
  let proof;
  try {
    if (addedRepair) ({ read: live, hashes: repairHashes } = projectRepair(workspace, readLive, override));
    proof = JSON.parse(live(AUDIT70_CI_INTEGRATION_PROOF));
    const scope = [...AUDIT70_CI_INTEGRATION_CHANGED, ...AUDIT70_CI_INTEGRATION_ADDED].sort();
    if (canonical(live(AUDIT70_CI_INTEGRATION_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "AUDIT70_CI_INTEGRATION"
        || proof.parentHead !== AUDIT70_CI_INTEGRATION_BASE || proof.planningParentHead !== AUDIT70_CI_INTEGRATION_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(AUDIT70_CI_INTEGRATION_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", AUDIT70_CI_INTEGRATION_BASE]).trim() !== AUDIT70_CI_INTEGRATION_BASE) fail("ancestor");
    const allowed = [...scope, AUDIT70_CI_INTEGRATION_PROOF].sort();
    const expected = [...parent.paths, ...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !(addedRepair && p === PROJECT_AUDIT_REPAIR_PROOF));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !(addedRepair && p === PROJECT_AUDIT_REPAIR_PROOF))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of AUDIT70_CI_INTEGRATION_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of AUDIT70_CI_INTEGRATION_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p) && !(addedRepair && p === PROJECT_AUDIT_REPAIR_PROOF))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: AUDIT70_CI_INTEGRATION_CHANGED,
    addedPaths: [...AUDIT70_CI_INTEGRATION_ADDED, AUDIT70_CI_INTEGRATION_PROOF, ...(addedRepair ? [PROJECT_AUDIT_REPAIR_PROOF] : [])],
    afterHashes: { ...Object.fromEntries([...AUDIT70_CI_INTEGRATION_CHANGED, ...AUDIT70_CI_INTEGRATION_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...repairHashes },
    read: sessionMemoRead(workspace, p => AUDIT70_CI_INTEGRATION_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function audit70CiIntegrationParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "audit70CiIntegrationParentReader", undefined,
    () => validate(root, undefined), AUDIT70_CI_INTEGRATION_PROOF);
}
