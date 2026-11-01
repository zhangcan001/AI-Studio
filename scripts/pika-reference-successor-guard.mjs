import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const PIKA_REFERENCE_BASE = "948afd5c73fc395a1742fe60c317ab2d62cdce5d";
export const PIKA_REFERENCE_PROOF = "docs/architecture/pika-reference-successor.json";
export const PIKA_REFERENCE_CHANGED = [
  "src-tauri/tests/dev082_unified_workflow_library.rs",
  "src/app/Phase7Retirement.test.tsx",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/ci-consolidate-fix-successor-guard.mjs",
  "src-tauri/src/application/builtin_runtime_packages.rs",
  "src-tauri/src/application/minimax_video_product_policy.rs",
  "src-tauri/src/application/product/h3_resolution.rs"
].sort();
export const PIKA_REFERENCE_ADDED = [
  "scripts/pika-reference-successor-guard.mjs",
  "scripts/testing/pika-reference.test.mjs",
  "src-tauri/src/application/builtin_pika_reference.rs"
].sort();
export const PIKA_REFERENCE_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "eightStepPikaBranchPinned": true,
  "referenceImagesMaxEight": true,
  "optionalVideoAudio": true,
  "emptyAndAudioOnlyRejected": true,
  "existingDefaultsPreserved": true
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
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function pikaReferenceScopeSince(root, base) {
  const tip = facts(root, PIKA_REFERENCE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...PIKA_REFERENCE_CHANGED, ...PIKA_REFERENCE_ADDED, PIKA_REFERENCE_PROOF,
  ])].sort();
}
export function pikaReferenceFixtureFiles(root) {
  return [...facts(root, PIKA_REFERENCE_BASE).paths, ...PIKA_REFERENCE_ADDED, PIKA_REFERENCE_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, PIKA_REFERENCE_BASE);
  const fail = id => violations.push("pika-reference-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(PIKA_REFERENCE_PROOF));
    const scope = [...PIKA_REFERENCE_CHANGED, ...PIKA_REFERENCE_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "PIKA_REFERENCE"
        || proof.parentHead !== PIKA_REFERENCE_BASE || proof.planningParentHead !== PIKA_REFERENCE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(PIKA_REFERENCE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", PIKA_REFERENCE_BASE]).trim() !== PIKA_REFERENCE_BASE) fail("ancestor");
    const allowed = [...scope, PIKA_REFERENCE_PROOF].sort();
    const expected = [...parent.paths, ...PIKA_REFERENCE_ADDED, PIKA_REFERENCE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of PIKA_REFERENCE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of PIKA_REFERENCE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: PIKA_REFERENCE_CHANGED,
    addedPaths: [...PIKA_REFERENCE_ADDED, PIKA_REFERENCE_PROOF],
    afterHashes: Object.fromEntries([...PIKA_REFERENCE_CHANGED, ...PIKA_REFERENCE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: sessionMemoRead(workspace, p => PIKA_REFERENCE_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function pikaReferenceParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "pikaReferenceParentReader", undefined,
    () => validate(root, undefined), PIKA_REFERENCE_PROOF);
}
