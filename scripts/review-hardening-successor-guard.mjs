import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const REVIEW_HARDENING_BASE = "f0aaf27a282b903475b104674bfb95cda5c795e1";
export const REVIEW_HARDENING_PROOF = "docs/architecture/review-hardening-successor.json";
export const REVIEW_HARDENING_CHANGED = [
  "scripts/single-shot-batch-control-successor-guard.mjs",
  "scripts/testing/stale-binding-migration.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
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
export function reviewHardeningScopeSince(root, base) {
  const tip = facts(root, REVIEW_HARDENING_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...REVIEW_HARDENING_CHANGED, ...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF,
  ])].sort();
}
export function reviewHardeningFixtureFiles(root) {
  return [...facts(root, REVIEW_HARDENING_BASE).paths, ...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, REVIEW_HARDENING_BASE);
  const fail = id => violations.push("review-hardening-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
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
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
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
  return { violations, changedPaths: REVIEW_HARDENING_CHANGED,
    addedPaths: [...REVIEW_HARDENING_ADDED, REVIEW_HARDENING_PROOF],
    afterHashes: Object.fromEntries([...REVIEW_HARDENING_CHANGED, ...REVIEW_HARDENING_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => REVIEW_HARDENING_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function reviewHardeningParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "reviewHardeningParentReader", undefined,
    () => validate(root, undefined), REVIEW_HARDENING_PROOF);
}
