import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionLiveRead } from "./boundary-validation-cache.mjs";

export const REFERENCE_IMAGES_IMPORT_BASE = "46739ad12c951f05c841075ccdf7e3941dfce3b7";
export const REFERENCE_IMAGES_IMPORT_PROOF = "docs/architecture/reference-images-import-successor.json";
export const REFERENCE_IMAGES_IMPORT_CHANGED = [
  "scripts/composition-kind-successor-guard.mjs",
  "src-tauri/src/application/mod.rs"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const REFERENCE_IMAGES_IMPORT_RUNTIME = [];
export const REFERENCE_IMAGES_IMPORT_ADDED = [
  "scripts/reference-images-import-successor-guard.mjs",
  "scripts/testing/reference-images-import.test.mjs"
].sort();
export const REFERENCE_IMAGES_IMPORT_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "referenceImagesOnlyRecipeImport": true,
  "pinnedContractStringsPreserved": true
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
export function referenceImagesImportScopeSince(root, base) {
  const tip = facts(root, REFERENCE_IMAGES_IMPORT_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...REFERENCE_IMAGES_IMPORT_CHANGED, ...REFERENCE_IMAGES_IMPORT_ADDED, REFERENCE_IMAGES_IMPORT_PROOF,
  ])].sort();
}
export function referenceImagesImportFixtureFiles(root) {
  return [...facts(root, REFERENCE_IMAGES_IMPORT_BASE).paths, ...REFERENCE_IMAGES_IMPORT_ADDED, REFERENCE_IMAGES_IMPORT_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, REFERENCE_IMAGES_IMPORT_BASE);
  const fail = id => violations.push("reference-images-import-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(REFERENCE_IMAGES_IMPORT_PROOF));
    const scope = [...REFERENCE_IMAGES_IMPORT_CHANGED, ...REFERENCE_IMAGES_IMPORT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "REFERENCE_IMAGES_IMPORT"
        || proof.parentHead !== REFERENCE_IMAGES_IMPORT_BASE || proof.planningParentHead !== REFERENCE_IMAGES_IMPORT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(REFERENCE_IMAGES_IMPORT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", REFERENCE_IMAGES_IMPORT_BASE]).trim() !== REFERENCE_IMAGES_IMPORT_BASE) fail("ancestor");
    const allowed = [...scope, REFERENCE_IMAGES_IMPORT_PROOF].sort();
    const expected = [...parent.paths, ...REFERENCE_IMAGES_IMPORT_ADDED, REFERENCE_IMAGES_IMPORT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of REFERENCE_IMAGES_IMPORT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of REFERENCE_IMAGES_IMPORT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of REFERENCE_IMAGES_IMPORT_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: REFERENCE_IMAGES_IMPORT_CHANGED,
    addedPaths: [...REFERENCE_IMAGES_IMPORT_ADDED, REFERENCE_IMAGES_IMPORT_PROOF],
    afterHashes: Object.fromEntries([...REFERENCE_IMAGES_IMPORT_CHANGED, ...REFERENCE_IMAGES_IMPORT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => REFERENCE_IMAGES_IMPORT_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function referenceImagesImportParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "referenceImagesImportParentReader", undefined,
    () => validate(root, undefined), REFERENCE_IMAGES_IMPORT_PROOF);
}
