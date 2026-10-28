import { workbenchPollClockParentReader, workbenchPollClockScopeSince, workbenchPollClockFixtureFiles, WORKBENCH_POLL_CLOCK_BASE } from './workbench-poll-clock-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const REFERENCE_DRAFT_BASE = "524794bc6d361e82427d81df54b4fbc3407cb40a";
export const REFERENCE_DRAFT_PROOF = "docs/architecture/reference-draft-recovery-successor.json";
export const REFERENCE_DRAFT_CHANGED = [
  "scripts/backup-package-bindings-successor-guard.mjs",
  "src/features/create/CreateInputs.tsx",
  "src/features/create/CreatePage.test.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const REFERENCE_DRAFT_RUNTIME = [];
export const REFERENCE_DRAFT_ADDED = [
  "scripts/reference-draft-recovery-successor-guard.mjs",
  "scripts/testing/reference-draft-recovery.test.mjs"
].sort();
export const REFERENCE_DRAFT_CONTRACTS = {
  "gpuExecuted": false,
  "schemaChanged": false,
  "backupFormatChanged": false,
  "backendChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true
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
export function referenceDraftScopeSince(root, base) {
  const tip = facts(root, REFERENCE_DRAFT_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...REFERENCE_DRAFT_CHANGED, ...REFERENCE_DRAFT_ADDED, REFERENCE_DRAFT_PROOF,
    ...workbenchPollClockScopeSince(root, WORKBENCH_POLL_CLOCK_BASE),
  ])].sort();
}
export function referenceDraftFixtureFiles(root) {
  return [...new Set([...facts(root, REFERENCE_DRAFT_BASE).paths, ...REFERENCE_DRAFT_ADDED, REFERENCE_DRAFT_PROOF, ...workbenchPollClockFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, REFERENCE_DRAFT_BASE);
  const fail = id => violations.push("reference-draft-recovery-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(REFERENCE_DRAFT_PROOF));
    const scope = [...REFERENCE_DRAFT_CHANGED, ...REFERENCE_DRAFT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "REFERENCE_DRAFT_RECOVERY"
        || proof.parentHead !== REFERENCE_DRAFT_BASE || proof.planningParentHead !== REFERENCE_DRAFT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(REFERENCE_DRAFT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", REFERENCE_DRAFT_BASE]).trim() !== REFERENCE_DRAFT_BASE) fail("ancestor");
    const allowed = [...scope, REFERENCE_DRAFT_PROOF].sort();
    const expected = [...parent.paths, ...REFERENCE_DRAFT_ADDED, REFERENCE_DRAFT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of REFERENCE_DRAFT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of REFERENCE_DRAFT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of REFERENCE_DRAFT_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...REFERENCE_DRAFT_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...REFERENCE_DRAFT_ADDED, REFERENCE_DRAFT_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...REFERENCE_DRAFT_CHANGED, ...REFERENCE_DRAFT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => REFERENCE_DRAFT_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function referenceDraftParentReader(root, override = undefined) {
  const check = () => { const successor = workbenchPollClockParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "referenceDraftParentReader", undefined, check, REFERENCE_DRAFT_PROOF);
}
