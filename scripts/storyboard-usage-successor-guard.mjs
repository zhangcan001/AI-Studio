import {usabilityCompletionParentReader, usabilityCompletionScopeSince, usabilityCompletionFixtureFiles, USABILITY_COMPLETION_BASE} from './usability-completion-successor-guard.mjs';
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const STORYBOARD_USAGE_BASE = "163499ba6e77ba4e0003b6af0c83abf08648926e";
export const STORYBOARD_USAGE_PROOF = "docs/architecture/storyboard-usage-successor.json";
export const STORYBOARD_USAGE_CHANGED = [
  "scripts/reference-authority-successor-guard.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "src/features/create/StoryboardSafety.test.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx"
].sort();
export const STORYBOARD_USAGE_ADDED = [
  "scripts/storyboard-usage-successor-guard.mjs",
  "scripts/testing/storyboard-usage.test.mjs"
].sort();
export const STORYBOARD_USAGE_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "manualOverridesPreserved": true,
  "saveBeforeGenerate": true,
  "startFailureFeedback": true,
  "exactRunNavigation": true,
  "singleSubmission": true
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
export function storyboardUsageScopeSince(root, base) {
  const tip = facts(root, STORYBOARD_USAGE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...STORYBOARD_USAGE_CHANGED, ...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF,
    ...usabilityCompletionScopeSince(root, USABILITY_COMPLETION_BASE),
  ])].sort();
}
export function storyboardUsageFixtureFiles(root) {
  return [...new Set([...facts(root, STORYBOARD_USAGE_BASE).paths, ...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF, ...usabilityCompletionFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, STORYBOARD_USAGE_BASE);
  const fail = id => violations.push("storyboard-usage-" + id);
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(STORYBOARD_USAGE_PROOF));
    const scope = [...STORYBOARD_USAGE_CHANGED, ...STORYBOARD_USAGE_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "STORYBOARD_USAGE"
        || proof.parentHead !== STORYBOARD_USAGE_BASE || proof.planningParentHead !== STORYBOARD_USAGE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(STORYBOARD_USAGE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", STORYBOARD_USAGE_BASE]).trim() !== STORYBOARD_USAGE_BASE) fail("ancestor");
    const allowed = [...scope, STORYBOARD_USAGE_PROOF].sort();
    const expected = [...parent.paths, ...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of STORYBOARD_USAGE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of STORYBOARD_USAGE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...STORYBOARD_USAGE_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...STORYBOARD_USAGE_ADDED, STORYBOARD_USAGE_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...STORYBOARD_USAGE_CHANGED, ...STORYBOARD_USAGE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => STORYBOARD_USAGE_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function storyboardUsageParentReader(root, override = undefined) {
  const check = () => { const successor = usabilityCompletionParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "storyboardUsageParentReader", undefined, check, STORYBOARD_USAGE_PROOF);
}
