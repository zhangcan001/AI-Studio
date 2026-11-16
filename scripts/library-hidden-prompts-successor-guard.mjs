import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const LIBRARY_HIDDEN_PROMPTS_BASE = "86acd5abbcb5f14eca34bdac505161d7e58d5804";
export const LIBRARY_HIDDEN_PROMPTS_PROOF = "docs/architecture/library-hidden-prompts-successor.json";
export const LIBRARY_HIDDEN_PROMPTS_CHANGED = [
  "scripts/run-recovery-polish-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/convenience.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/production-safety.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/run-recovery-polish.test.mjs",
  "scripts/testing/storyboard-feedback.test.mjs",
  "scripts/testing/storyboard-integrity.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "scripts/testing/usage-polish.test.mjs",
  "src/features/library/LibraryFindability.test.tsx",
  "src/features/library/LibraryPage.tsx"
].sort();
export const LIBRARY_HIDDEN_PROMPTS_ADDED = [
  "scripts/library-hidden-prompts-successor-guard.mjs",
  "scripts/testing/library-hidden-prompts.test.mjs"
].sort();
export const LIBRARY_HIDDEN_PROMPTS_CONTRACTS = {
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
export function libraryHiddenPromptsScopeSince(root, base) {
  const tip = facts(root, LIBRARY_HIDDEN_PROMPTS_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...LIBRARY_HIDDEN_PROMPTS_CHANGED, ...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF,
  ])].sort();
}
export function libraryHiddenPromptsFixtureFiles(root) {
  return [...facts(root, LIBRARY_HIDDEN_PROMPTS_BASE).paths, ...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, LIBRARY_HIDDEN_PROMPTS_BASE);
  const fail = id => violations.push("library-hidden-prompts-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(LIBRARY_HIDDEN_PROMPTS_PROOF));
    const scope = [...LIBRARY_HIDDEN_PROMPTS_CHANGED, ...LIBRARY_HIDDEN_PROMPTS_ADDED].sort();
    if (canonical(live(LIBRARY_HIDDEN_PROMPTS_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "LIBRARY_HIDDEN_PROMPTS"
        || proof.parentHead !== LIBRARY_HIDDEN_PROMPTS_BASE || proof.planningParentHead !== LIBRARY_HIDDEN_PROMPTS_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(LIBRARY_HIDDEN_PROMPTS_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", LIBRARY_HIDDEN_PROMPTS_BASE]).trim() !== LIBRARY_HIDDEN_PROMPTS_BASE) fail("ancestor");
    const allowed = [...scope, LIBRARY_HIDDEN_PROMPTS_PROOF].sort();
    const expected = [...parent.paths, ...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of LIBRARY_HIDDEN_PROMPTS_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of LIBRARY_HIDDEN_PROMPTS_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: LIBRARY_HIDDEN_PROMPTS_CHANGED,
    addedPaths: [...LIBRARY_HIDDEN_PROMPTS_ADDED, LIBRARY_HIDDEN_PROMPTS_PROOF],
    afterHashes: Object.fromEntries([...LIBRARY_HIDDEN_PROMPTS_CHANGED, ...LIBRARY_HIDDEN_PROMPTS_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: sessionMemoRead(workspace, p => LIBRARY_HIDDEN_PROMPTS_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function libraryHiddenPromptsParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "libraryHiddenPromptsParentReader", undefined,
    () => validate(root, undefined), LIBRARY_HIDDEN_PROMPTS_PROOF);
}
