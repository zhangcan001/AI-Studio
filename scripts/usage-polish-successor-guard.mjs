import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const USAGE_POLISH_BASE = "ee7ee271212d73363f877c977e52f80986e93494";
export const USAGE_POLISH_PROOF = "docs/architecture/usage-polish-successor.json";
export const USAGE_POLISH_CHANGED = [
  "scripts/convenience-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/convenience.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "src-tauri/src/application/artifact_service.rs",
  "src-tauri/src/application/product/creation_facade.rs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/support/creation_context_contract.rs",
  "src-tauri/tests/support/run_workspace_contract.rs",
  "src/features/create/CreateResults.tsx",
  "src/features/create/CreateVideoWorkspace.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/storyboardModel.ts",
  "src/features/runs/RunResults.tsx",
  "src/features/runs/RunsController.ts",
  "src/features/runs/RunsPage.test.tsx",
  "src/features/shots/ShotBulkConfigPanel.tsx",
  "src/features/shots/StoryboardBatchDialog.test.tsx",
  "src/features/shots/StoryboardBatchDialog.tsx",
  "src/product/client.test.ts",
  "src/product/client.ts",
  "src/product/errors.ts",
  "src/product/transport.ts",
  "src/product/types.ts",
  "src/stores/studioStore.ts"
].sort();
export const USAGE_POLISH_ADDED = [
  "scripts/testing/usage-polish.test.mjs",
  "scripts/usage-polish-successor-guard.mjs"
].sort();
export const USAGE_POLISH_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "explicitSelectionAndReview": true,
  "boundedWorkbenchPaging": true,
  "noImplicitStart": true
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
export function usagePolishScopeSince(root, base) {
  const tip = facts(root, USAGE_POLISH_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...USAGE_POLISH_CHANGED, ...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF,
  ])].sort();
}
export function usagePolishFixtureFiles(root) {
  return [...facts(root, USAGE_POLISH_BASE).paths, ...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, USAGE_POLISH_BASE);
  const fail = id => violations.push("usage-polish-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(USAGE_POLISH_PROOF));
    const scope = [...USAGE_POLISH_CHANGED, ...USAGE_POLISH_ADDED].sort();
    if (canonical(live(USAGE_POLISH_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "USAGE_POLISH"
        || proof.parentHead !== USAGE_POLISH_BASE || proof.planningParentHead !== USAGE_POLISH_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(USAGE_POLISH_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", USAGE_POLISH_BASE]).trim() !== USAGE_POLISH_BASE) fail("ancestor");
    const allowed = [...scope, USAGE_POLISH_PROOF].sort();
    const expected = [...parent.paths, ...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of USAGE_POLISH_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of USAGE_POLISH_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: USAGE_POLISH_CHANGED,
    addedPaths: [...USAGE_POLISH_ADDED, USAGE_POLISH_PROOF],
    afterHashes: Object.fromEntries([...USAGE_POLISH_CHANGED, ...USAGE_POLISH_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: sessionMemoRead(workspace, p => USAGE_POLISH_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function usagePolishParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "usagePolishParentReader", undefined,
    () => validate(root, undefined), USAGE_POLISH_PROOF);
}
