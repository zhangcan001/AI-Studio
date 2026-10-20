import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from "./boundary-validation-cache.mjs";

export const ENTITY_REFERENCE_AUTO_APPLY_BASE = "d183d7c711ac2aa516a182dbcb5e12d8a58665f3";
export const ENTITY_REFERENCE_AUTO_APPLY_PROOF = "docs/architecture/entity-reference-auto-apply-successor.json";
export const ENTITY_REFERENCE_AUTO_APPLY_CHANGED = [
  "scripts/retired-ui-code-cleanup-successor-guard.mjs",
  "src/features/create/CreateComposer.test.tsx",
  "src/features/create/CreateComposer.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const ENTITY_REFERENCE_AUTO_APPLY_RUNTIME = [];
export const ENTITY_REFERENCE_AUTO_APPLY_ADDED = [
  "docs/architecture/entity-reference-auto-apply.md",
  "scripts/entity-reference-auto-apply-successor-guard.mjs",
  "scripts/testing/entity-reference-auto-apply.test.mjs"
].sort();
export const ENTITY_REFERENCE_AUTO_APPLY_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "queueAuthorityChanged": false,
  "gpuExecuted": false,
  "exactRecipeInputsPreserved": true,
  "explicitGenerationRequired": true,
  "occAndProjectIsolationPreserved": true,
  "automaticRetriesDisabled": true
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
export function entityReferenceAutoApplyScopeSince(root, base) {
  const tip = facts(root, ENTITY_REFERENCE_AUTO_APPLY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...ENTITY_REFERENCE_AUTO_APPLY_CHANGED, ...ENTITY_REFERENCE_AUTO_APPLY_ADDED, ENTITY_REFERENCE_AUTO_APPLY_PROOF,
  ])].sort();
}
export function entityReferenceAutoApplyFixtureFiles(root) {
  return [...facts(root, ENTITY_REFERENCE_AUTO_APPLY_BASE).paths, ...ENTITY_REFERENCE_AUTO_APPLY_ADDED, ENTITY_REFERENCE_AUTO_APPLY_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, ENTITY_REFERENCE_AUTO_APPLY_BASE);
  const fail = id => violations.push("entity-reference-auto-apply-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : readFileSync(join(workspace, p), "utf8"));
  let proof;
  try {
    proof = JSON.parse(live(ENTITY_REFERENCE_AUTO_APPLY_PROOF));
    const scope = [...ENTITY_REFERENCE_AUTO_APPLY_CHANGED, ...ENTITY_REFERENCE_AUTO_APPLY_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "ENTITY_REFERENCE_AUTO_APPLY"
        || proof.parentHead !== ENTITY_REFERENCE_AUTO_APPLY_BASE || proof.planningParentHead !== ENTITY_REFERENCE_AUTO_APPLY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(ENTITY_REFERENCE_AUTO_APPLY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", ENTITY_REFERENCE_AUTO_APPLY_BASE]).trim() !== ENTITY_REFERENCE_AUTO_APPLY_BASE) fail("ancestor");
    const allowed = [...scope, ENTITY_REFERENCE_AUTO_APPLY_PROOF].sort();
    const expected = [...parent.paths, ...ENTITY_REFERENCE_AUTO_APPLY_ADDED, ENTITY_REFERENCE_AUTO_APPLY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of ENTITY_REFERENCE_AUTO_APPLY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of ENTITY_REFERENCE_AUTO_APPLY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of ENTITY_REFERENCE_AUTO_APPLY_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: ENTITY_REFERENCE_AUTO_APPLY_CHANGED,
    addedPaths: [...ENTITY_REFERENCE_AUTO_APPLY_ADDED, ENTITY_REFERENCE_AUTO_APPLY_PROOF],
    afterHashes: Object.fromEntries([...ENTITY_REFERENCE_AUTO_APPLY_CHANGED, ...ENTITY_REFERENCE_AUTO_APPLY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => ENTITY_REFERENCE_AUTO_APPLY_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function entityReferenceAutoApplyParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "entityReferenceAutoApplyParentReader", undefined,
    () => validate(root, undefined), ENTITY_REFERENCE_AUTO_APPLY_PROOF);
}
