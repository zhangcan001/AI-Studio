import { referenceDraftParentReader, referenceDraftScopeSince, referenceDraftFixtureFiles, REFERENCE_DRAFT_BASE } from './reference-draft-recovery-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const BACKUP_PACKAGE_BASE = "05abb6e8f3766b5d95c24149a6e706350b1cab37";
export const BACKUP_PACKAGE_PROOF = "docs/architecture/backup-package-bindings-successor.json";
export const BACKUP_PACKAGE_CHANGED = [
  "scripts/storyboard-atomic-row-successor-guard.mjs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/infrastructure/database/repositories/project_backup.rs"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const BACKUP_PACKAGE_RUNTIME = [];
export const BACKUP_PACKAGE_ADDED = [
  "scripts/backup-package-bindings-successor-guard.mjs",
  "scripts/testing/backup-package-bindings.test.mjs"
].sort();
export const BACKUP_PACKAGE_CONTRACTS = {
  "schemaChanged": false,
  "gpuExecuted": false,
  "historicalProofBytesUnchanged": true,
  "runtimeBytesChanged": false,
  "additiveV21Provenance": true,
  "backupVersion": 21,
  "backupFormatChanged": true
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
export function backupPackageScopeSince(root, base) {
  const tip = facts(root, BACKUP_PACKAGE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...BACKUP_PACKAGE_CHANGED, ...BACKUP_PACKAGE_ADDED, BACKUP_PACKAGE_PROOF,
    ...referenceDraftScopeSince(root, REFERENCE_DRAFT_BASE),
  ])].sort();
}
export function backupPackageFixtureFiles(root) {
  return [...new Set([...facts(root, BACKUP_PACKAGE_BASE).paths, ...BACKUP_PACKAGE_ADDED, BACKUP_PACKAGE_PROOF, ...referenceDraftFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, BACKUP_PACKAGE_BASE);
  const fail = id => violations.push("backup-package-bindings-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(BACKUP_PACKAGE_PROOF));
    const scope = [...BACKUP_PACKAGE_CHANGED, ...BACKUP_PACKAGE_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "BACKUP_PACKAGE_BINDINGS"
        || proof.parentHead !== BACKUP_PACKAGE_BASE || proof.planningParentHead !== BACKUP_PACKAGE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(BACKUP_PACKAGE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", BACKUP_PACKAGE_BASE]).trim() !== BACKUP_PACKAGE_BASE) fail("ancestor");
    const allowed = [...scope, BACKUP_PACKAGE_PROOF].sort();
    const expected = [...parent.paths, ...BACKUP_PACKAGE_ADDED, BACKUP_PACKAGE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of BACKUP_PACKAGE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of BACKUP_PACKAGE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of BACKUP_PACKAGE_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...BACKUP_PACKAGE_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...BACKUP_PACKAGE_ADDED, BACKUP_PACKAGE_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...BACKUP_PACKAGE_CHANGED, ...BACKUP_PACKAGE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => BACKUP_PACKAGE_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function backupPackageParentReader(root, override = undefined) {
  const check = () => { const successor = referenceDraftParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "backupPackageParentReader", undefined, check, BACKUP_PACKAGE_PROOF);
}
