import { promptSectionLoadParentReader, promptSectionLoadScopeSince, promptSectionLoadFixtureFiles, PROMPT_SECTION_LOAD_BASE } from './prompt-section-load-successor-guard.mjs';
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const DELETE_INTEGRITY_BASE = "6866df4b76c5566a51abdd583c2af2f5c90c3d3a";
export const DELETE_INTEGRITY_PROOF = "docs/architecture/delete-integrity-successor.json";
export const DELETE_INTEGRITY_CHANGED = [
  "scripts/testing/cancel-correctness.test.mjs",
  "scripts/testing/review-hardening.test.mjs",
  "scripts/testing/single-shot-batch-control.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "src-tauri/src/application/asset_deletion_service.rs",
  "src-tauri/src/application/ports/asset_deletion_repository.rs",
  "src-tauri/src/application/production_queue_service.rs",
  "src-tauri/src/application/shot_service.rs",
  "src-tauri/src/infrastructure/database/repositories/asset_deletion.rs",
  "src-tauri/src/infrastructure/database/repositories/production_queue.rs",
  "src-tauri/src/infrastructure/database/repositories/shot.rs",
  "src-tauri/tests/dev052_runtime_integration.rs"
].sort();
export const DELETE_INTEGRITY_ADDED = [
  "scripts/delete-integrity-successor-guard.mjs",
  "scripts/testing/delete-integrity.test.mjs"
].sort();
export const DELETE_INTEGRITY_CONTRACTS = {
  "schemaChanged": false,
  "migrationAdded": false,
  "guardsRelaxed": false,
  "gpuExecuted": false,
  "productionQueueOnlyExecution": true,
  "storyEntityAssetsBlockDeletion": true,
  "retryParentsBlockDeletion": true,
  "shotDeleteChecksActiveBindingsAtomically": true
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
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function deleteIntegrityScopeSince(root, base) {
  const tip = facts(root, DELETE_INTEGRITY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...DELETE_INTEGRITY_CHANGED, ...DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_PROOF,
    ...promptSectionLoadScopeSince(root, PROMPT_SECTION_LOAD_BASE),
  ])].sort();
}
export function deleteIntegrityFixtureFiles(root) {
  return [...new Set([...facts(root, DELETE_INTEGRITY_BASE).paths, ...DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_PROOF, ...promptSectionLoadFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, DELETE_INTEGRITY_BASE);
  const fail = id => violations.push("delete-integrity-" + id);
  // Project only independently validated successor bytes; forged overrides still fail.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(DELETE_INTEGRITY_PROOF));
    const scope = [...DELETE_INTEGRITY_CHANGED, ...DELETE_INTEGRITY_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "DELETE_INTEGRITY"
        || proof.parentHead !== DELETE_INTEGRITY_BASE || proof.planningParentHead !== DELETE_INTEGRITY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(DELETE_INTEGRITY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", DELETE_INTEGRITY_BASE]).trim() !== DELETE_INTEGRITY_BASE) fail("ancestor");
    const allowed = [...scope, DELETE_INTEGRITY_PROOF].sort();
    const expected = [...parent.paths, ...DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of DELETE_INTEGRITY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of DELETE_INTEGRITY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...DELETE_INTEGRITY_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...DELETE_INTEGRITY_ADDED, DELETE_INTEGRITY_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...DELETE_INTEGRITY_CHANGED, ...DELETE_INTEGRITY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => DELETE_INTEGRITY_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function deleteIntegrityParentReader(root, override = undefined) {
  const check = () => { const successor = promptSectionLoadParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "deleteIntegrityParentReader", undefined, check, DELETE_INTEGRITY_PROOF);
}
