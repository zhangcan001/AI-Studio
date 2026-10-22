import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionLiveRead } from "./boundary-validation-cache.mjs";

export const GUARD_CHAIN_PERF_BASE = "86cd696723c03dd46ec97abcf7e650e5a1672dab";
export const GUARD_CHAIN_PERF_PROOF = "docs/architecture/guard-chain-perf-successor.json";
export const GUARD_CHAIN_PERF_CHANGED = [
  "scripts/boundary-validation-cache.mjs",
  "scripts/prop-kind-import-successor-guard.mjs"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const GUARD_CHAIN_PERF_RUNTIME = [];
export const GUARD_CHAIN_PERF_ADDED = [
  "scripts/guard-chain-perf-successor-guard.mjs",
  "scripts/testing/guard-chain-perf.test.mjs"
].sort();
export const GUARD_CHAIN_PERF_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": false,
  "guardSemanticsUnchanged": true,
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
export function guardChainPerfScopeSince(root, base) {
  const tip = facts(root, GUARD_CHAIN_PERF_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...GUARD_CHAIN_PERF_CHANGED, ...GUARD_CHAIN_PERF_ADDED, GUARD_CHAIN_PERF_PROOF,
  ])].sort();
}
export function guardChainPerfFixtureFiles(root) {
  return [...facts(root, GUARD_CHAIN_PERF_BASE).paths, ...GUARD_CHAIN_PERF_ADDED, GUARD_CHAIN_PERF_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, GUARD_CHAIN_PERF_BASE);
  const fail = id => violations.push("guard-chain-perf-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(GUARD_CHAIN_PERF_PROOF));
    const scope = [...GUARD_CHAIN_PERF_CHANGED, ...GUARD_CHAIN_PERF_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "GUARD_CHAIN_PERF"
        || proof.parentHead !== GUARD_CHAIN_PERF_BASE || proof.planningParentHead !== GUARD_CHAIN_PERF_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(GUARD_CHAIN_PERF_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", GUARD_CHAIN_PERF_BASE]).trim() !== GUARD_CHAIN_PERF_BASE) fail("ancestor");
    const allowed = [...scope, GUARD_CHAIN_PERF_PROOF].sort();
    const expected = [...parent.paths, ...GUARD_CHAIN_PERF_ADDED, GUARD_CHAIN_PERF_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of GUARD_CHAIN_PERF_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of GUARD_CHAIN_PERF_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of GUARD_CHAIN_PERF_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: GUARD_CHAIN_PERF_CHANGED,
    addedPaths: [...GUARD_CHAIN_PERF_ADDED, GUARD_CHAIN_PERF_PROOF],
    afterHashes: Object.fromEntries([...GUARD_CHAIN_PERF_CHANGED, ...GUARD_CHAIN_PERF_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => GUARD_CHAIN_PERF_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function guardChainPerfParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "guardChainPerfParentReader", undefined,
    () => validate(root, undefined), GUARD_CHAIN_PERF_PROOF);
}
