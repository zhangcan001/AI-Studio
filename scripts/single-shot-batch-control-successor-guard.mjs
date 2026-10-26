import { reviewHardeningParentReader, reviewHardeningScopeSince, reviewHardeningFixtureFiles, REVIEW_HARDENING_BASE } from './review-hardening-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const SINGLE_SHOT_BATCH_CONTROL_BASE = "8ba8be37ca9c500ac9a14f852b468c14696a6eed";
export const SINGLE_SHOT_BATCH_CONTROL_PROOF = "docs/architecture/single-shot-batch-control-successor.json";
export const SINGLE_SHOT_BATCH_CONTROL_CHANGED = [
  "scripts/veda-removed-white-flash-successor-guard.mjs",
  "src-tauri/src/application/product/run_facade/detail.rs",
  "src-tauri/tests/support/run_workspace_contract.rs",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/runs/RunDetail.tsx",
  "src/features/runs/RunsController.ts",
  "src/features/runs/RunsPage.test.tsx",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "src/features/shots/FolderBatchImportPanel.tsx",
  "src/product/types.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const SINGLE_SHOT_BATCH_CONTROL_RUNTIME = [];
export const SINGLE_SHOT_BATCH_CONTROL_ADDED = [
  "scripts/single-shot-batch-control-successor-guard.mjs",
  "scripts/testing/single-shot-batch-control.test.mjs"
].sort();
export const SINGLE_SHOT_BATCH_CONTROL_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "perInputShotIdentityFromFormalLinks": true,
  "ambiguousShotIdentityNotGuessed": true,
  "singleItemRetryUsesExistingQueue": true,
  "frozenSnapshotsPreserved": true,
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
export function singleShotBatchControlScopeSince(root, base) {
  const tip = facts(root, SINGLE_SHOT_BATCH_CONTROL_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...SINGLE_SHOT_BATCH_CONTROL_CHANGED, ...SINGLE_SHOT_BATCH_CONTROL_ADDED, SINGLE_SHOT_BATCH_CONTROL_PROOF,
    ...reviewHardeningScopeSince(root, REVIEW_HARDENING_BASE),
  ])].sort();
}
export function singleShotBatchControlFixtureFiles(root) {
  return [...new Set([...facts(root, SINGLE_SHOT_BATCH_CONTROL_BASE).paths, ...SINGLE_SHOT_BATCH_CONTROL_ADDED, SINGLE_SHOT_BATCH_CONTROL_PROOF, ...reviewHardeningFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, SINGLE_SHOT_BATCH_CONTROL_BASE);
  const fail = id => violations.push("single-shot-batch-control-" + id);
  // Project only independently validated successor bytes; forged overrides still fail.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(SINGLE_SHOT_BATCH_CONTROL_PROOF));
    const scope = [...SINGLE_SHOT_BATCH_CONTROL_CHANGED, ...SINGLE_SHOT_BATCH_CONTROL_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "SINGLE_SHOT_BATCH_CONTROL"
        || proof.parentHead !== SINGLE_SHOT_BATCH_CONTROL_BASE || proof.planningParentHead !== SINGLE_SHOT_BATCH_CONTROL_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(SINGLE_SHOT_BATCH_CONTROL_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", SINGLE_SHOT_BATCH_CONTROL_BASE]).trim() !== SINGLE_SHOT_BATCH_CONTROL_BASE) fail("ancestor");
    const allowed = [...scope, SINGLE_SHOT_BATCH_CONTROL_PROOF].sort();
    const expected = [...parent.paths, ...SINGLE_SHOT_BATCH_CONTROL_ADDED, SINGLE_SHOT_BATCH_CONTROL_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of SINGLE_SHOT_BATCH_CONTROL_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of SINGLE_SHOT_BATCH_CONTROL_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of SINGLE_SHOT_BATCH_CONTROL_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...SINGLE_SHOT_BATCH_CONTROL_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...SINGLE_SHOT_BATCH_CONTROL_ADDED, SINGLE_SHOT_BATCH_CONTROL_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...SINGLE_SHOT_BATCH_CONTROL_CHANGED, ...SINGLE_SHOT_BATCH_CONTROL_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => SINGLE_SHOT_BATCH_CONTROL_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function singleShotBatchControlParentReader(root, override = undefined) {
  const successor = reviewHardeningParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "singleShotBatchControlParentReader", undefined, check, SINGLE_SHOT_BATCH_CONTROL_PROOF);
}
