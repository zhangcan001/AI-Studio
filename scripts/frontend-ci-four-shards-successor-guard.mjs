import { vedaVerboseDiagnosticsParentReader, vedaVerboseDiagnosticsScopeSince, vedaVerboseDiagnosticsFixtureFiles, VEDA_VERBOSE_DIAGNOSTICS_BASE } from './veda-verbose-diagnostics-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const FRONTEND_CI_FOUR_SHARDS_BASE = "89f9ec15968a62bb7fa175d86352244cb2ddaa8b";
export const FRONTEND_CI_FOUR_SHARDS_PROOF = "docs/architecture/frontend-ci-four-shards-successor.json";
export const FRONTEND_CI_FOUR_SHARDS_CHANGED = [
  // Session-scoped memoization of identical Git inventory/ancestor queries
  // across the guard chain (boundary-validation-cache sessionGit). No check removed.
  ".github/workflows/ci.yml",
  ".github/workflows/frontend-only-ci.yml",
  "scripts/backup-package-bindings-successor-guard.mjs",
  "scripts/batch-create-reason-successor-guard.mjs",
  "scripts/boundary-validation-cache.mjs",
  "scripts/composition-detail-hint-successor-guard.mjs",
  "scripts/composition-kind-successor-guard.mjs",
  "scripts/composition-scene-anchor-successor-guard.mjs",
  "scripts/composition-usage-successor-guard.mjs",
  "scripts/default-reference-generator-successor-guard.mjs",
  "scripts/desktop-visual-refinement-successor-guard.mjs",
  "scripts/entity-category-library-successor-guard.mjs",
  "scripts/entity-reference-auto-apply-successor-guard.mjs",
  "scripts/folder-batch-import-successor-guard.mjs",
  "scripts/folder-import-entity-dedupe-successor-guard.mjs",
  "scripts/generation-progress-refresh-successor-guard.mjs",
  "scripts/guard-chain-perf-successor-guard.mjs",
  "scripts/h3-composition-layout-successor-guard.mjs",
  "scripts/h3-prompt-sections-successor-guard.mjs",
  "scripts/h3-standard-reference-successor-guard.mjs",
  "scripts/h3-veda-reference-successor-guard.mjs",
  "scripts/library-confirm-ci-race-successor-guard.mjs",
  "scripts/prop-kind-import-successor-guard.mjs",
  "scripts/reference-draft-recovery-successor-guard.mjs",
  "scripts/reference-images-import-successor-guard.mjs",
  "scripts/retired-ui-code-cleanup-successor-guard.mjs",
  "scripts/shot-duration-cap-successor-guard.mjs",
  "scripts/storyboard-atomic-row-successor-guard.mjs",
  "scripts/storyboard-load-recovery-successor-guard.mjs",
  "scripts/task-cancel-delete-successor-guard.mjs",
  "scripts/testing/ui-phase7b-r1-metadata.test.mjs",
  "scripts/ui-phase7b-backup-prompt-successor-guard.mjs",
  "scripts/ui-phase7b-r1-metadata-successor-guard.mjs",
  "scripts/ui-phase7c-frontend-ci-budget-successor-guard.mjs",
  "scripts/user-copy-cleanup-successor-guard.mjs",
  "scripts/veda-reference-media-successor-guard.mjs",
  "scripts/workbench-poll-clock-successor-guard.mjs",
  "scripts/workbench-visual-refinement-successor-guard.mjs",
  "scripts/workflow-authoring-retirement-successor-guard.mjs",
  "src/app/UiPhase5ECiShardBoundary.test.ts",
  "src/app/UiPhase5FCiMemoryBoundary.test.ts",
  "src/app/UiPhase5ICiWorkerTeardownBoundary.test.ts",
  "src/app/UiPhase5JCiPerFileWorkerBoundary.test.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const FRONTEND_CI_FOUR_SHARDS_RUNTIME = [];
export const FRONTEND_CI_FOUR_SHARDS_ADDED = [
  "scripts/frontend-ci-four-shards-successor-guard.mjs",
  "scripts/testing/frontend-ci-four-shards.test.mjs"
].sort();
export const FRONTEND_CI_FOUR_SHARDS_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": false,
  "frontendShardCount": 4,
  "allFrontendTestsStillRun": true,
  "boundaryShardsUnchanged": true,
  "guardsRelaxed": false,
  "sessionGitMemoizedWithinOneValidation": true
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
      blobs.set(p, immutableBlobBytes(header[1], () => bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function frontendCiFourShardsScopeSince(root, base) {
  const tip = facts(root, FRONTEND_CI_FOUR_SHARDS_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...FRONTEND_CI_FOUR_SHARDS_CHANGED, ...FRONTEND_CI_FOUR_SHARDS_ADDED, FRONTEND_CI_FOUR_SHARDS_PROOF,
    ...vedaVerboseDiagnosticsScopeSince(root, VEDA_VERBOSE_DIAGNOSTICS_BASE),
  ])].sort();
}
export function frontendCiFourShardsFixtureFiles(root) {
  return [...new Set([...facts(root, FRONTEND_CI_FOUR_SHARDS_BASE).paths, ...FRONTEND_CI_FOUR_SHARDS_ADDED, FRONTEND_CI_FOUR_SHARDS_PROOF, ...vedaVerboseDiagnosticsFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, FRONTEND_CI_FOUR_SHARDS_BASE);
  const fail = id => violations.push("frontend-ci-four-shards-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(FRONTEND_CI_FOUR_SHARDS_PROOF));
    const scope = [...FRONTEND_CI_FOUR_SHARDS_CHANGED, ...FRONTEND_CI_FOUR_SHARDS_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "FRONTEND_CI_FOUR_SHARDS"
        || proof.parentHead !== FRONTEND_CI_FOUR_SHARDS_BASE || proof.planningParentHead !== FRONTEND_CI_FOUR_SHARDS_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(FRONTEND_CI_FOUR_SHARDS_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", FRONTEND_CI_FOUR_SHARDS_BASE]).trim() !== FRONTEND_CI_FOUR_SHARDS_BASE) fail("ancestor");
    const allowed = [...scope, FRONTEND_CI_FOUR_SHARDS_PROOF].sort();
    const expected = [...parent.paths, ...FRONTEND_CI_FOUR_SHARDS_ADDED, FRONTEND_CI_FOUR_SHARDS_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of FRONTEND_CI_FOUR_SHARDS_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of FRONTEND_CI_FOUR_SHARDS_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of FRONTEND_CI_FOUR_SHARDS_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...FRONTEND_CI_FOUR_SHARDS_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...FRONTEND_CI_FOUR_SHARDS_ADDED, FRONTEND_CI_FOUR_SHARDS_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...FRONTEND_CI_FOUR_SHARDS_CHANGED, ...FRONTEND_CI_FOUR_SHARDS_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => FRONTEND_CI_FOUR_SHARDS_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function frontendCiFourShardsParentReader(root, override = undefined) {
  const check = () => { const successor = vedaVerboseDiagnosticsParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "frontendCiFourShardsParentReader", undefined, check, FRONTEND_CI_FOUR_SHARDS_PROOF);
}
