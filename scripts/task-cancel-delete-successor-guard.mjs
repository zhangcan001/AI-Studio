import { folderImportEntityDedupeParentReader, folderImportEntityDedupeScopeSince, folderImportEntityDedupeFixtureFiles, FOLDER_IMPORT_ENTITY_DEDUPE_BASE } from './folder-import-entity-dedupe-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const TASK_CANCEL_DELETE_BASE = "b57cc14bd33825e1fdc3e5e3a18f43b0b4327319";
export const TASK_CANCEL_DELETE_PROOF = "docs/architecture/task-cancel-delete-successor.json";
export const TASK_CANCEL_DELETE_CHANGED = [
  "scripts/h3-prompt-sections-successor-guard.mjs",
  "src-tauri/src/application/ports/production_queue_repository.rs",
  "src-tauri/src/application/ports/task_repository.rs",
  "src-tauri/src/application/product/run_facade.rs",
  "src-tauri/src/application/product/run_facade/workspace.rs",
  "src-tauri/src/application/production_queue_service.rs",
  "src-tauri/src/application/task_cancellation_service.rs",
  "src-tauri/src/application/task_recovery_service.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/commands/production_queue.rs",
  "src-tauri/src/commands/task.rs",
  "src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs",
  "src-tauri/src/infrastructure/database/repositories/production_queue.rs",
  "src-tauri/src/infrastructure/database/repositories/task.rs",
  "src-tauri/src/infrastructure/database/repositories/task_history.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/dev040_safety.rs",
  "src-tauri/tests/dev052_runtime_integration.rs",
  "src-tauri/tests/dev055_release_compatibility.rs",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/production/ProductionQueueDrawer.test.tsx",
  "src/features/production/ProductionQueueDrawer.tsx",
  "src/features/runs/RunDetail.tsx",
  "src/features/runs/RunsController.ts",
  "src/features/runs/RunsPhase6C.test.tsx",
  "src/features/runs/runRecoveryPresentation.ts",
  "src/features/runs/runsModel.ts",
  "src/features/shots/ShotWorkspace.tsx",
  "src/features/shots/hooks/useShotQueueController.ts",
  "src/features/studio/ProductionQueuePanel.tsx",
  "src/product/client.ts",
  "src/product/transport.ts",
  "src/services/tauriClient.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const TASK_CANCEL_DELETE_RUNTIME = [];
export const TASK_CANCEL_DELETE_ADDED = [
  "scripts/task-cancel-delete-successor-guard.mjs",
  "scripts/testing/task-cancel-delete.test.mjs",
  "src-tauri/migrations/050_task_record_hidden.sql"
].sort();
export const TASK_CANCEL_DELETE_CONTRACTS = {
  "schemaChanged": true,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "taskRecordHiddenMigration050": true,
  "cancelInterruptsRunningTask": true,
  "deleteKeepsGeneratedFiles": true,
  "staleDispatchedCancellable": true,
  "pinnedContractStringsPreserved": true
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
    const blobs = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, bytes.subarray(start, start + size)); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, canonical(blobs.get(p).toString("utf8")));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function taskCancelDeleteScopeSince(root, base) {
  const tip = facts(root, TASK_CANCEL_DELETE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...TASK_CANCEL_DELETE_CHANGED, ...TASK_CANCEL_DELETE_ADDED, TASK_CANCEL_DELETE_PROOF,
    ...folderImportEntityDedupeScopeSince(root, FOLDER_IMPORT_ENTITY_DEDUPE_BASE),
  ])].sort();
}
export function taskCancelDeleteFixtureFiles(root) {
  return [...new Set([...facts(root, TASK_CANCEL_DELETE_BASE).paths, ...TASK_CANCEL_DELETE_ADDED, TASK_CANCEL_DELETE_PROOF, ...folderImportEntityDedupeFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, TASK_CANCEL_DELETE_BASE);
  const fail = id => violations.push("task-cancel-delete-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(TASK_CANCEL_DELETE_PROOF));
    const scope = [...TASK_CANCEL_DELETE_CHANGED, ...TASK_CANCEL_DELETE_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "TASK_CANCEL_DELETE"
        || proof.parentHead !== TASK_CANCEL_DELETE_BASE || proof.planningParentHead !== TASK_CANCEL_DELETE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(TASK_CANCEL_DELETE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", TASK_CANCEL_DELETE_BASE]).trim() !== TASK_CANCEL_DELETE_BASE) fail("ancestor");
    const allowed = [...scope, TASK_CANCEL_DELETE_PROOF].sort();
    const expected = [...parent.paths, ...TASK_CANCEL_DELETE_ADDED, TASK_CANCEL_DELETE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of TASK_CANCEL_DELETE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of TASK_CANCEL_DELETE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of TASK_CANCEL_DELETE_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...TASK_CANCEL_DELETE_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...TASK_CANCEL_DELETE_ADDED, TASK_CANCEL_DELETE_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...TASK_CANCEL_DELETE_CHANGED, ...TASK_CANCEL_DELETE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => TASK_CANCEL_DELETE_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function taskCancelDeleteParentReader(root, override = undefined) {
  const successor = folderImportEntityDedupeParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "taskCancelDeleteParentReader", undefined, check, TASK_CANCEL_DELETE_PROOF);
}
