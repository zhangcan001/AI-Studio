import { generationProgressRefreshParentReader, generationProgressRefreshScopeSince, generationProgressRefreshFixtureFiles, GENERATION_PROGRESS_REFRESH_BASE } from './generation-progress-refresh-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead } from "./boundary-validation-cache.mjs";

export const WORKFLOW_AUTHORING_RETIREMENT_BASE = "448f0f079f3234084163225d0e16eed70dbba836";
export const WORKFLOW_AUTHORING_RETIREMENT_PROOF = "docs/architecture/workflow-authoring-retirement-successor.json";
export const WORKFLOW_AUTHORING_RETIREMENT_CHANGED = [
  "scripts/h3-veda-reference-successor-guard.mjs",
  "scripts/style-boundary-guard.mjs",
  "scripts/workbench-visual-refinement-successor-guard.mjs",
  "src/app/App.tsx",
  "src/app/M1ReadinessBoundary.test.ts",
  "src/app/Phase7Retirement.test.tsx",
  "src/app/StudioShell.test.tsx",
  "src/app/WorkflowNavigationUat.test.tsx",
  "src/app/v3/AppShellV3.tsx",
  "src/app/v3/VideoShell.test.tsx",
  "src/features/create/CreateController.ts",
  "src/features/create/CreatePage.test.tsx",
  "src/features/create/CreatePage.tsx",
  "src/features/generators/GeneratorSettingsPage.test.tsx",
  "src/features/generators/GeneratorSettingsPage.tsx",
  "src/features/settings/SettingsWorkspace.test.tsx",
  "src/features/settings/SettingsWorkspace.tsx",
  "src/features/workflow-lab/WorkflowLabBoundary.test.tsx",
  "src/features/workflow-lab/WorkflowLabPage.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const WORKFLOW_AUTHORING_RETIREMENT_RUNTIME = [];
export const WORKFLOW_AUTHORING_RETIREMENT_ADDED = [
  "docs/architecture/workflow-authoring-retirement.md",
  "scripts/testing/workflow-authoring-retirement.test.mjs",
  "scripts/workflow-authoring-retirement-successor-guard.mjs"
].sort();
export const WORKFLOW_AUTHORING_RETIREMENT_CONTRACTS = {
  "backupFormatChanged": false,
  "authoringUiRetired": true,
  "gpuExecuted": false,
  "historicalProofBytesUnchanged": true,
  "existingBindingsPreserved": true,
  "legacyRoutesReadOnly": true,
  "schemaChanged": false,
  "runtimeBytesChanged": false
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
      blobs.set(p, bytes.subarray(start, start + size)); blobIds.set(p, header[1]); offset = start + size + 1;
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
export function workflowAuthoringRetirementScopeSince(root, base) {
  const tip = facts(root, WORKFLOW_AUTHORING_RETIREMENT_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...WORKFLOW_AUTHORING_RETIREMENT_CHANGED, ...WORKFLOW_AUTHORING_RETIREMENT_ADDED, WORKFLOW_AUTHORING_RETIREMENT_PROOF,
    ...generationProgressRefreshScopeSince(root, GENERATION_PROGRESS_REFRESH_BASE),
  ])].sort();
}
export function workflowAuthoringRetirementFixtureFiles(root) {
  return [...new Set([...facts(root, WORKFLOW_AUTHORING_RETIREMENT_BASE).paths, ...WORKFLOW_AUTHORING_RETIREMENT_ADDED, WORKFLOW_AUTHORING_RETIREMENT_PROOF, ...generationProgressRefreshFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, WORKFLOW_AUTHORING_RETIREMENT_BASE);
  const fail = id => violations.push("workflow-authoring-retirement-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(WORKFLOW_AUTHORING_RETIREMENT_PROOF));
    const scope = [...WORKFLOW_AUTHORING_RETIREMENT_CHANGED, ...WORKFLOW_AUTHORING_RETIREMENT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "WORKFLOW_AUTHORING_RETIREMENT"
        || proof.parentHead !== WORKFLOW_AUTHORING_RETIREMENT_BASE || proof.planningParentHead !== WORKFLOW_AUTHORING_RETIREMENT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(WORKFLOW_AUTHORING_RETIREMENT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", WORKFLOW_AUTHORING_RETIREMENT_BASE]).trim() !== WORKFLOW_AUTHORING_RETIREMENT_BASE) fail("ancestor");
    const allowed = [...scope, WORKFLOW_AUTHORING_RETIREMENT_PROOF].sort();
    const expected = [...parent.paths, ...WORKFLOW_AUTHORING_RETIREMENT_ADDED, WORKFLOW_AUTHORING_RETIREMENT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of WORKFLOW_AUTHORING_RETIREMENT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of WORKFLOW_AUTHORING_RETIREMENT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of WORKFLOW_AUTHORING_RETIREMENT_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...WORKFLOW_AUTHORING_RETIREMENT_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...WORKFLOW_AUTHORING_RETIREMENT_ADDED, WORKFLOW_AUTHORING_RETIREMENT_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...WORKFLOW_AUTHORING_RETIREMENT_CHANGED, ...WORKFLOW_AUTHORING_RETIREMENT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => WORKFLOW_AUTHORING_RETIREMENT_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function workflowAuthoringRetirementParentReader(root, override = undefined) {
  const check = () => { const successor = generationProgressRefreshParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "workflowAuthoringRetirementParentReader", undefined, check, WORKFLOW_AUTHORING_RETIREMENT_PROOF);
}
