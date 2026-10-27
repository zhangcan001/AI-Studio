import { successorChainParentReader } from './successor-chain-successor-guard.mjs';
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const STORYBOARD_WORKBENCH_BATCH_BASE = "bff74b7400732c8feee9612e7105d3d46ef37d1c";
export const STORYBOARD_WORKBENCH_BATCH_PROOF = "docs/architecture/storyboard-workbench-batch-successor.json";
export const STORYBOARD_WORKBENCH_BATCH_CHANGED = [
  "scripts/testing/cancel-correctness.test.mjs",
  "scripts/testing/delete-integrity.test.mjs",
  "scripts/testing/package-import-lock.test.mjs",
  "scripts/testing/prompt-section-load.test.mjs",
  "scripts/testing/prompt-sections-without-at.test.mjs",
  "scripts/testing/review-hardening.test.mjs",
  "scripts/testing/shot-save-atomic.test.mjs",
  "scripts/testing/single-shot-batch-control.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/lib.rs",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/product/client.ts",
  "src/product/transport.ts",
  "src/product/types.ts"
].sort();
export const STORYBOARD_WORKBENCH_BATCH_ADDED = [
  "scripts/storyboard-workbench-batch-successor-guard.mjs",
  "scripts/testing/storyboard-workbench-batch.test.mjs"
].sort();
export const STORYBOARD_WORKBENCH_BATCH_CONTRACTS = {
  "schemaChanged": false,
  "migrationAdded": false,
  "guardsRelaxed": false,
  "gpuExecuted": false,
  "productionQueueOnlyExecution": true
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
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function storyboardWorkbenchBatchScopeSince(root, base) {
  const tip = facts(root, STORYBOARD_WORKBENCH_BATCH_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...STORYBOARD_WORKBENCH_BATCH_CHANGED, ...STORYBOARD_WORKBENCH_BATCH_ADDED, STORYBOARD_WORKBENCH_BATCH_PROOF,
  ])].sort();
}
export function storyboardWorkbenchBatchFixtureFiles(root) {
  return [...facts(root, STORYBOARD_WORKBENCH_BATCH_BASE).paths, ...STORYBOARD_WORKBENCH_BATCH_ADDED, STORYBOARD_WORKBENCH_BATCH_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, STORYBOARD_WORKBENCH_BATCH_BASE);
  const fail = id => violations.push("storyboard-workbench-batch-" + id);
  // Project only independently validated successor bytes; forged overrides still fail.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(STORYBOARD_WORKBENCH_BATCH_PROOF));
    const scope = [...STORYBOARD_WORKBENCH_BATCH_CHANGED, ...STORYBOARD_WORKBENCH_BATCH_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "STORYBOARD_WORKBENCH_BATCH"
        || proof.parentHead !== STORYBOARD_WORKBENCH_BATCH_BASE || proof.planningParentHead !== STORYBOARD_WORKBENCH_BATCH_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(STORYBOARD_WORKBENCH_BATCH_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", STORYBOARD_WORKBENCH_BATCH_BASE]).trim() !== STORYBOARD_WORKBENCH_BATCH_BASE) fail("ancestor");
    const allowed = [...scope, STORYBOARD_WORKBENCH_BATCH_PROOF].sort();
    const expected = [...parent.paths, ...STORYBOARD_WORKBENCH_BATCH_ADDED, STORYBOARD_WORKBENCH_BATCH_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of STORYBOARD_WORKBENCH_BATCH_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of STORYBOARD_WORKBENCH_BATCH_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...STORYBOARD_WORKBENCH_BATCH_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...STORYBOARD_WORKBENCH_BATCH_ADDED, STORYBOARD_WORKBENCH_BATCH_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...STORYBOARD_WORKBENCH_BATCH_CHANGED, ...STORYBOARD_WORKBENCH_BATCH_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => STORYBOARD_WORKBENCH_BATCH_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function storyboardWorkbenchBatchParentReader(root, override = undefined) {
  const successor = successorChainParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "storyboardWorkbenchBatchParentReader", undefined, check, STORYBOARD_WORKBENCH_BATCH_PROOF);
}
