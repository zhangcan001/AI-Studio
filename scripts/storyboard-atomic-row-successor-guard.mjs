import { backupPackageParentReader, backupPackageScopeSince, backupPackageFixtureFiles, BACKUP_PACKAGE_BASE } from './backup-package-bindings-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from "./boundary-validation-cache.mjs";

export const STORYBOARD_ATOMIC_BASE = "94a67e5a48c7f8ffe11353145a0b2542b3c506f7";
export const STORYBOARD_ATOMIC_PROOF = "docs/architecture/storyboard-atomic-row-successor.json";
export const STORYBOARD_ATOMIC_CHANGED = [
  "scripts/storyboard-load-recovery-successor-guard.mjs",
  "src-tauri/src/application/ports/mod.rs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/application/shot_service.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/infrastructure/database/repositories/mod.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/product_library_contract.rs",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/product/client.test.ts",
  "src/product/client.ts",
  "src/product/transport.ts",
  "src/product/types.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const STORYBOARD_ATOMIC_RUNTIME = [];
export const STORYBOARD_ATOMIC_ADDED = [
  "scripts/storyboard-atomic-row-successor-guard.mjs",
  "scripts/testing/storyboard-atomic-row.test.mjs"
].sort();
export const STORYBOARD_ATOMIC_CONTRACTS = {
  "backupFormatChanged": false,
  "backendChanged": true,
  "historicalProofBytesUnchanged": true,
  "runtimeBytesChanged": false,
  "schemaChanged": false,
  "gpuExecuted": false
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const git = (root, args) => execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
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
export function storyboardAtomicScopeSince(root, base) {
  const tip = facts(root, STORYBOARD_ATOMIC_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...STORYBOARD_ATOMIC_CHANGED, ...STORYBOARD_ATOMIC_ADDED, STORYBOARD_ATOMIC_PROOF,
    ...backupPackageScopeSince(root, BACKUP_PACKAGE_BASE),
  ])].sort();
}
export function storyboardAtomicFixtureFiles(root) {
  return [...new Set([...facts(root, STORYBOARD_ATOMIC_BASE).paths, ...STORYBOARD_ATOMIC_ADDED, STORYBOARD_ATOMIC_PROOF, ...backupPackageFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, STORYBOARD_ATOMIC_BASE);
  const fail = id => violations.push("storyboard-atomic-row-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(STORYBOARD_ATOMIC_PROOF));
    const scope = [...STORYBOARD_ATOMIC_CHANGED, ...STORYBOARD_ATOMIC_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "STORYBOARD_ATOMIC_ROW"
        || proof.parentHead !== STORYBOARD_ATOMIC_BASE || proof.planningParentHead !== STORYBOARD_ATOMIC_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(STORYBOARD_ATOMIC_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", STORYBOARD_ATOMIC_BASE]).trim() !== STORYBOARD_ATOMIC_BASE) fail("ancestor");
    const allowed = [...scope, STORYBOARD_ATOMIC_PROOF].sort();
    const expected = [...parent.paths, ...STORYBOARD_ATOMIC_ADDED, STORYBOARD_ATOMIC_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of STORYBOARD_ATOMIC_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of STORYBOARD_ATOMIC_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of STORYBOARD_ATOMIC_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...STORYBOARD_ATOMIC_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...STORYBOARD_ATOMIC_ADDED, STORYBOARD_ATOMIC_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...STORYBOARD_ATOMIC_CHANGED, ...STORYBOARD_ATOMIC_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => STORYBOARD_ATOMIC_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function storyboardAtomicParentReader(root, override = undefined) {
  const successor = backupPackageParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "storyboardAtomicParentReader", undefined, check, STORYBOARD_ATOMIC_PROOF);
}
