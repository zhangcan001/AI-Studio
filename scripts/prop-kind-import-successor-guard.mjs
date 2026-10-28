import { guardChainPerfParentReader, guardChainPerfScopeSince, guardChainPerfFixtureFiles, GUARD_CHAIN_PERF_BASE } from './guard-chain-perf-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const PROP_KIND_IMPORT_BASE = "0c4c208ac8e1b4559bea60cfb200661d38e18117";
export const PROP_KIND_IMPORT_PROOF = "docs/architecture/prop-kind-import-successor.json";
export const PROP_KIND_IMPORT_CHANGED = [
  "docs/architecture/folder-batch-import.md",
  "scripts/user-copy-cleanup-successor-guard.mjs",
  "src-tauri/src/application/mod.rs",
  "src/features/shots/FolderBatchImportPanel.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const PROP_KIND_IMPORT_RUNTIME = [];
export const PROP_KIND_IMPORT_ADDED = [
  "scripts/prop-kind-import-successor-guard.mjs",
  "scripts/testing/prop-kind-import.test.mjs"
].sort();
export const PROP_KIND_IMPORT_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "manifestPropKindOnly": true,
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
export function propKindImportScopeSince(root, base) {
  const tip = facts(root, PROP_KIND_IMPORT_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...PROP_KIND_IMPORT_CHANGED, ...PROP_KIND_IMPORT_ADDED, PROP_KIND_IMPORT_PROOF,
    ...guardChainPerfScopeSince(root, GUARD_CHAIN_PERF_BASE),
  ])].sort();
}
export function propKindImportFixtureFiles(root) {
  return [...new Set([...facts(root, PROP_KIND_IMPORT_BASE).paths, ...PROP_KIND_IMPORT_ADDED, PROP_KIND_IMPORT_PROOF, ...guardChainPerfFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, PROP_KIND_IMPORT_BASE);
  const fail = id => violations.push("prop-kind-import-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(PROP_KIND_IMPORT_PROOF));
    const scope = [...PROP_KIND_IMPORT_CHANGED, ...PROP_KIND_IMPORT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "PROP_KIND_IMPORT"
        || proof.parentHead !== PROP_KIND_IMPORT_BASE || proof.planningParentHead !== PROP_KIND_IMPORT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(PROP_KIND_IMPORT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", PROP_KIND_IMPORT_BASE]).trim() !== PROP_KIND_IMPORT_BASE) fail("ancestor");
    const allowed = [...scope, PROP_KIND_IMPORT_PROOF].sort();
    const expected = [...parent.paths, ...PROP_KIND_IMPORT_ADDED, PROP_KIND_IMPORT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of PROP_KIND_IMPORT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of PROP_KIND_IMPORT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of PROP_KIND_IMPORT_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...PROP_KIND_IMPORT_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...PROP_KIND_IMPORT_ADDED, PROP_KIND_IMPORT_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...PROP_KIND_IMPORT_CHANGED, ...PROP_KIND_IMPORT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => PROP_KIND_IMPORT_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function propKindImportParentReader(root, override = undefined) {
  const check = () => { const successor = guardChainPerfParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "propKindImportParentReader", undefined, check, PROP_KIND_IMPORT_PROOF);
}
