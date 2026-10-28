import { propKindImportParentReader, propKindImportScopeSince, propKindImportFixtureFiles, PROP_KIND_IMPORT_BASE } from './prop-kind-import-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const USER_COPY_CLEANUP_BASE = "d7f4ec7d46b6395df072eec69a834f03d8409cab";
export const USER_COPY_CLEANUP_PROOF = "docs/architecture/user-copy-cleanup-successor.json";
export const USER_COPY_CLEANUP_CHANGED = [
  "scripts/default-reference-generator-successor-guard.mjs",
  "src-tauri/src/application/mod.rs",
  "src/features/assets/AssetPreview.tsx",
  "src/features/prompts/PromptStudio.tsx",
  "src/features/runs/RunsPage.tsx",
  "src/features/runs/runsModel.ts",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "src/features/shots/FolderBatchImportPanel.tsx",
  "src/features/shots/ShotReadinessInspector.test.tsx",
  "src/features/shots/ShotReadinessInspector.tsx",
  "src/features/shots/useFolderBatchImportController.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const USER_COPY_CLEANUP_RUNTIME = [];
export const USER_COPY_CLEANUP_ADDED = [
  "scripts/user-copy-cleanup-successor-guard.mjs",
  "scripts/testing/user-copy-cleanup.test.mjs"
].sort();
export const USER_COPY_CLEANUP_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "userFacingCopyOnly": true,
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
export function userCopyCleanupScopeSince(root, base) {
  const tip = facts(root, USER_COPY_CLEANUP_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...USER_COPY_CLEANUP_CHANGED, ...USER_COPY_CLEANUP_ADDED, USER_COPY_CLEANUP_PROOF,
    ...propKindImportScopeSince(root, PROP_KIND_IMPORT_BASE),
  ])].sort();
}
export function userCopyCleanupFixtureFiles(root) {
  return [...new Set([...facts(root, USER_COPY_CLEANUP_BASE).paths, ...USER_COPY_CLEANUP_ADDED, USER_COPY_CLEANUP_PROOF, ...propKindImportFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, USER_COPY_CLEANUP_BASE);
  const fail = id => violations.push("user-copy-cleanup-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(USER_COPY_CLEANUP_PROOF));
    const scope = [...USER_COPY_CLEANUP_CHANGED, ...USER_COPY_CLEANUP_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "USER_COPY_CLEANUP"
        || proof.parentHead !== USER_COPY_CLEANUP_BASE || proof.planningParentHead !== USER_COPY_CLEANUP_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(USER_COPY_CLEANUP_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", USER_COPY_CLEANUP_BASE]).trim() !== USER_COPY_CLEANUP_BASE) fail("ancestor");
    const allowed = [...scope, USER_COPY_CLEANUP_PROOF].sort();
    const expected = [...parent.paths, ...USER_COPY_CLEANUP_ADDED, USER_COPY_CLEANUP_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of USER_COPY_CLEANUP_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of USER_COPY_CLEANUP_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of USER_COPY_CLEANUP_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...USER_COPY_CLEANUP_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...USER_COPY_CLEANUP_ADDED, USER_COPY_CLEANUP_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...USER_COPY_CLEANUP_CHANGED, ...USER_COPY_CLEANUP_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => USER_COPY_CLEANUP_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function userCopyCleanupParentReader(root, override = undefined) {
  const check = () => { const successor = propKindImportParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "userCopyCleanupParentReader", undefined, check, USER_COPY_CLEANUP_PROOF);
}
