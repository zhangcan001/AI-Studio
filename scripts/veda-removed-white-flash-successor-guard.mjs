import { singleShotBatchControlParentReader, singleShotBatchControlScopeSince, singleShotBatchControlFixtureFiles, SINGLE_SHOT_BATCH_CONTROL_BASE } from './single-shot-batch-control-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const VEDA_REMOVED_WHITE_FLASH_BASE = "181886b36c7ad8867d83b7a74e55af2328f14da6";
export const VEDA_REMOVED_WHITE_FLASH_PROOF = "docs/architecture/veda-removed-white-flash-successor.json";
export const VEDA_REMOVED_WHITE_FLASH_CHANGED = [
  // Built-in Veda reference 1.3.0: 1.2.0 without the VedaSparseAttention node
  // 147, BasicGuider 126 reads LoRA node 145 directly (same-seed A/B: Veda
  // causes the frame-119 white flash); approved supersession 1.2.0 -> 1.3.0.
  "scripts/superseded-import-pin-successor-guard.mjs",
  "src-tauri/src/application/builtin_runtime_packages.rs",
  "src-tauri/src/application/minimax_video_product_policy.rs",
  "src-tauri/tests/dev082_unified_workflow_library.rs",
  "src-tauri/tests/support/minimax_video_policy_regressions.rs"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const VEDA_REMOVED_WHITE_FLASH_RUNTIME = [];
export const VEDA_REMOVED_WHITE_FLASH_ADDED = [
  "scripts/veda-removed-white-flash-successor-guard.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs"
].sort();
export const VEDA_REMOVED_WHITE_FLASH_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "migrationAdded": false,
  "newBuiltinPackage": "minimax_h3_reference_veda_turbo_1_3_0",
  "onlyVedaNodeRemoved": true,
  "historicalVedaPackagesUnchanged": true,
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
      blobs.set(p, bytes.subarray(start, start + size)); blobIds.set(p, header[1]); offset = start + size + 1;
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
export function vedaRemovedWhiteFlashScopeSince(root, base) {
  const tip = facts(root, VEDA_REMOVED_WHITE_FLASH_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...VEDA_REMOVED_WHITE_FLASH_CHANGED, ...VEDA_REMOVED_WHITE_FLASH_ADDED, VEDA_REMOVED_WHITE_FLASH_PROOF,
    ...singleShotBatchControlScopeSince(root, SINGLE_SHOT_BATCH_CONTROL_BASE),
  ])].sort();
}
export function vedaRemovedWhiteFlashFixtureFiles(root) {
  return [...new Set([...facts(root, VEDA_REMOVED_WHITE_FLASH_BASE).paths, ...VEDA_REMOVED_WHITE_FLASH_ADDED, VEDA_REMOVED_WHITE_FLASH_PROOF, ...singleShotBatchControlFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, VEDA_REMOVED_WHITE_FLASH_BASE);
  const fail = id => violations.push("veda-removed-white-flash-" + id);
  // Project only independently validated successor bytes; forged overrides still fail.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(VEDA_REMOVED_WHITE_FLASH_PROOF));
    const scope = [...VEDA_REMOVED_WHITE_FLASH_CHANGED, ...VEDA_REMOVED_WHITE_FLASH_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "VEDA_REMOVED_WHITE_FLASH"
        || proof.parentHead !== VEDA_REMOVED_WHITE_FLASH_BASE || proof.planningParentHead !== VEDA_REMOVED_WHITE_FLASH_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(VEDA_REMOVED_WHITE_FLASH_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", VEDA_REMOVED_WHITE_FLASH_BASE]).trim() !== VEDA_REMOVED_WHITE_FLASH_BASE) fail("ancestor");
    const allowed = [...scope, VEDA_REMOVED_WHITE_FLASH_PROOF].sort();
    const expected = [...parent.paths, ...VEDA_REMOVED_WHITE_FLASH_ADDED, VEDA_REMOVED_WHITE_FLASH_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of VEDA_REMOVED_WHITE_FLASH_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of VEDA_REMOVED_WHITE_FLASH_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of VEDA_REMOVED_WHITE_FLASH_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...VEDA_REMOVED_WHITE_FLASH_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...VEDA_REMOVED_WHITE_FLASH_ADDED, VEDA_REMOVED_WHITE_FLASH_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...VEDA_REMOVED_WHITE_FLASH_CHANGED, ...VEDA_REMOVED_WHITE_FLASH_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => VEDA_REMOVED_WHITE_FLASH_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function vedaRemovedWhiteFlashParentReader(root, override = undefined) {
  const check = () => { const successor = singleShotBatchControlParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "vedaRemovedWhiteFlashParentReader", undefined, check, VEDA_REMOVED_WHITE_FLASH_PROOF);
}
