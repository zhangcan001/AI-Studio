import { scopeSinceChainScopeSince, scopeSinceChainFixtureFiles, SCOPE_SINCE_CHAIN_BASE } from './scope-since-chain-successor-guard.mjs';
import { cancelCorrectnessParentReader, cancelCorrectnessScopeSince, cancelCorrectnessFixtureFiles, CANCEL_CORRECTNESS_BASE } from './cancel-correctness-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const REVIEW_HARDENING_BASE = "f0aaf27a282b903475b104674bfb95cda5c795e1";
export const REVIEW_HARDENING_PROOF = "docs/architecture/review-hardening-successor.json";
export const REVIEW_HARDENING_CHANGED = [
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
  "scripts/frontend-ci-four-shards-successor-guard.mjs",
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
  "scripts/single-shot-batch-control-successor-guard.mjs",
  "scripts/stale-binding-migration-successor-guard.mjs",
  "scripts/storyboard-atomic-row-successor-guard.mjs",
  "scripts/storyboard-load-recovery-successor-guard.mjs",
  "scripts/superseded-import-pin-successor-guard.mjs",
  "scripts/task-cancel-delete-successor-guard.mjs",
  "scripts/testing/stale-binding-migration.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "scripts/ui-phase7b-backup-prompt-successor-guard.mjs",
  "scripts/ui-phase7b-r1-metadata-successor-guard.mjs",
  "scripts/user-copy-cleanup-successor-guard.mjs",
  "scripts/veda-reference-media-successor-guard.mjs",
  "scripts/veda-removed-white-flash-successor-guard.mjs",
  "scripts/veda-verbose-diagnostics-successor-guard.mjs",
  "scripts/workbench-poll-clock-successor-guard.mjs",
  "scripts/workbench-visual-refinement-successor-guard.mjs",
  "scripts/workflow-authoring-retirement-successor-guard.mjs",
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/infrastructure/database/repositories/workflow_library.rs",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const REVIEW_HARDENING_RUNTIME = [];
export const REVIEW_HARDENING_ADDED = [
  "scripts/review-hardening-successor-guard.mjs",
  "scripts/testing/review-hardening.test.mjs"
].sort();
export const REVIEW_HARDENING_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "manifestErrorsNameField": true,
  "migrationUsesCurrentTime": true,
  "migrationDropsUndeclaredInputs": true,
  "openShotFailureShownInChinese": true,
  "sharedImmutableBlobTextCache": true,
  "timeLimitsRaised": false,
  "guardsRelaxed": false
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
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function reviewHardeningScopeSince(root, base) {
  const tip = facts(root, REVIEW_HARDENING_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...REVIEW_HARDENING_CHANGED, ...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF,
    // Jump to the live tip ScopeSince (covers cancel…scope-since-chain) to keep Boundary probes fast.
    ...scopeSinceChainScopeSince(root, REVIEW_HARDENING_BASE),
  ])].sort();
}
export function reviewHardeningFixtureFiles(root) {
  return [...new Set([...facts(root, REVIEW_HARDENING_BASE).paths, ...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF, ...scopeSinceChainFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, REVIEW_HARDENING_BASE);
  const fail = id => violations.push("review-hardening-" + id);
  // Project only independently validated successor bytes; forged overrides still fail.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(REVIEW_HARDENING_PROOF));
    const scope = [...REVIEW_HARDENING_CHANGED, ...REVIEW_HARDENING_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "REVIEW_HARDENING"
        || proof.parentHead !== REVIEW_HARDENING_BASE || proof.planningParentHead !== REVIEW_HARDENING_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(REVIEW_HARDENING_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", REVIEW_HARDENING_BASE]).trim() !== REVIEW_HARDENING_BASE) fail("ancestor");
    const allowed = [...scope, REVIEW_HARDENING_PROOF].sort();
    const expected = [...parent.paths, ...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of REVIEW_HARDENING_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of REVIEW_HARDENING_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of REVIEW_HARDENING_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...REVIEW_HARDENING_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...REVIEW_HARDENING_CHANGED, ...REVIEW_HARDENING_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => REVIEW_HARDENING_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function reviewHardeningParentReader(root, override = undefined) {
  const check = () => { const successor = cancelCorrectnessParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "reviewHardeningParentReader", undefined, check, REVIEW_HARDENING_PROOF);
}
