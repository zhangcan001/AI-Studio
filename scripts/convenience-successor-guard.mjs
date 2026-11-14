import { usagePolishParentReader, usagePolishScopeSince, usagePolishFixtureFiles, USAGE_POLISH_BASE } from "./usage-polish-successor-guard.mjs";
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const CONVENIENCE_BASE = "203a9c7ef660f53a504b841014d45475c65d9a6c";
export const CONVENIENCE_PROOF = "docs/architecture/convenience-successor.json";
export const CONVENIENCE_CHANGED = [
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "scripts/usage-flow-successor-guard.mjs",
  "src-tauri/src/application/product/selected_export.rs",
  "src-tauri/src/commands/product.rs",
  "src/app/App.tsx",
  "src/app/NormalProductPages.tsx",
  "src/components/ResultFileActions.tsx",
  "src/features/create/CreatePage.css",
  "src/features/create/ShotReviewDialog.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/UsageFlowSafety.test.tsx",
  "src/features/create/storyboardModel.ts",
  "src/features/library/LibraryController.ts",
  "src/features/library/LibraryDetail.tsx",
  "src/features/library/LibraryPage.test.tsx",
  "src/features/library/LibraryPage.tsx",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/library/StoryEntityCategorySection.tsx",
  "src/features/shots/ShotBulkConfigPanel.tsx",
  "src/features/shots/StoryboardBatchDialog.tsx",
  "src/product/client.test.ts",
  "src/product/client.ts",
  "src/product/transport.ts",
  "src/product/types.ts"
].sort();
export const CONVENIENCE_ADDED = [
  "scripts/convenience-successor-guard.mjs",
  "scripts/testing/convenience.test.mjs",
  "src/features/shots/StoryboardBatchDialog.test.tsx"
].sort();
export const CONVENIENCE_CONTRACTS = {
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
export function convenienceScopeSince(root, base) {
  const tip = facts(root, CONVENIENCE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...CONVENIENCE_CHANGED, ...CONVENIENCE_ADDED, CONVENIENCE_PROOF,
    ...usagePolishScopeSince(root, USAGE_POLISH_BASE),
  ])].sort();
}
export function convenienceFixtureFiles(root) {
  return [...new Set([...facts(root, CONVENIENCE_BASE).paths, ...CONVENIENCE_ADDED, CONVENIENCE_PROOF, ...usagePolishFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const successor = usagePolishParentReader(root, override);
  const workspace = resolve(root), violations = [...successor.violations], parent = facts(workspace, CONVENIENCE_BASE);
  const fail = id => violations.push("convenience-" + id);
  const live = p => canonical(successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(CONVENIENCE_PROOF));
    const scope = [...CONVENIENCE_CHANGED, ...CONVENIENCE_ADDED].sort();
    if (canonical(live(CONVENIENCE_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "CONVENIENCE"
        || proof.parentHead !== CONVENIENCE_BASE || proof.planningParentHead !== CONVENIENCE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(CONVENIENCE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", CONVENIENCE_BASE]).trim() !== CONVENIENCE_BASE) fail("ancestor");
    const allowed = [...scope, CONVENIENCE_PROOF].sort();
    const expected = [...parent.paths, ...CONVENIENCE_ADDED, CONVENIENCE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of CONVENIENCE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of CONVENIENCE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...CONVENIENCE_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...CONVENIENCE_ADDED, CONVENIENCE_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...CONVENIENCE_CHANGED, ...CONVENIENCE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => CONVENIENCE_CHANGED.includes(p) ? parent.read(p) : successor.read(p)) };
}
export function convenienceParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "convenienceParentReader", undefined,
    () => validate(root, undefined), CONVENIENCE_PROOF);
}
