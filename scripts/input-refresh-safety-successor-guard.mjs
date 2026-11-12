import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const INPUT_REFRESH_SAFETY_BASE = "e90b617a51ffdabe12a3159f6348190701bfb3f6";
export const INPUT_REFRESH_SAFETY_PROOF = "docs/architecture/input-refresh-safety-successor.json";
export const INPUT_REFRESH_SAFETY_CHANGED = [
  "scripts/create-rerun-safety-successor-guard.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "src/features/create/CreateController.ts",
  "src/features/create/CreateVideoWorkspace.test.tsx",
  "src/features/create/usePersistentVideoInputs.test.tsx",
  "src/features/create/usePersistentVideoInputs.ts"
].sort();
export const INPUT_REFRESH_SAFETY_ADDED = [
  "scripts/input-refresh-safety-successor-guard.mjs",
  "scripts/testing/input-refresh-safety.test.mjs"
].sort();
export const INPUT_REFRESH_SAFETY_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "semanticRequestRetryIdentity": true,
  "backgroundErrorRecovery": true,
  "loadingSubscriptionFreshness": true,
  "dirtyOccConflictPreserved": true
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
export function inputRefreshSafetyScopeSince(root, base) {
  const tip = facts(root, INPUT_REFRESH_SAFETY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...INPUT_REFRESH_SAFETY_CHANGED, ...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF,
  ])].sort();
}
export function inputRefreshSafetyFixtureFiles(root) {
  return [...facts(root, INPUT_REFRESH_SAFETY_BASE).paths, ...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, INPUT_REFRESH_SAFETY_BASE);
  const fail = id => violations.push("input-refresh-safety-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(INPUT_REFRESH_SAFETY_PROOF));
    const scope = [...INPUT_REFRESH_SAFETY_CHANGED, ...INPUT_REFRESH_SAFETY_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "INPUT_REFRESH_SAFETY"
        || proof.parentHead !== INPUT_REFRESH_SAFETY_BASE || proof.planningParentHead !== INPUT_REFRESH_SAFETY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(INPUT_REFRESH_SAFETY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", INPUT_REFRESH_SAFETY_BASE]).trim() !== INPUT_REFRESH_SAFETY_BASE) fail("ancestor");
    const allowed = [...scope, INPUT_REFRESH_SAFETY_PROOF].sort();
    const expected = [...parent.paths, ...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of INPUT_REFRESH_SAFETY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of INPUT_REFRESH_SAFETY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: INPUT_REFRESH_SAFETY_CHANGED,
    addedPaths: [...INPUT_REFRESH_SAFETY_ADDED, INPUT_REFRESH_SAFETY_PROOF],
    afterHashes: Object.fromEntries([...INPUT_REFRESH_SAFETY_CHANGED, ...INPUT_REFRESH_SAFETY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: sessionMemoRead(workspace, p => INPUT_REFRESH_SAFETY_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function inputRefreshSafetyParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "inputRefreshSafetyParentReader", undefined,
    () => validate(root, undefined), INPUT_REFRESH_SAFETY_PROOF);
}
