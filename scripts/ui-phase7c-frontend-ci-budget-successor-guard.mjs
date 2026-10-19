import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from "./boundary-validation-cache.mjs";

export const PHASE7C_PARENT = "f3b8c6708dd9b18702b9effd61fdf3d407dd179b";
export const PHASE7C_PROOF = "docs/architecture/ui-phase7c-frontend-ci-budget-successor.json";
export const PHASE7C_CHANGED = [
  "scripts/style-boundary-guard.mjs",
  // Pure memoization of immutable parent blobs and commit:path reads so the
  // fail-closed 90s Windows Boundary budget holds. No check is removed.
  "scripts/ui-phase5-ci-repair-successor-guard.mjs",
  "scripts/ui-phase5a-storyboard-successor-guard.mjs",
  "scripts/ui-phase5b-mixed-video-successor-guard.mjs",
  "scripts/ui-phase5c-review-successor-guard.mjs",
  "scripts/ui-phase5d-ci-timeout-successor-guard.mjs",
  "scripts/ui-phase5e-ci-shard-successor-guard.mjs",
  "scripts/ui-phase5f-ci-memory-successor-guard.mjs",
  "scripts/ui-phase5g-ci-timeout-successor-guard.mjs",
  "scripts/ui-phase5h-ci-timeout-budget-successor-guard.mjs",
  "scripts/ui-phase5i-ci-worker-teardown-successor-guard.mjs",
  "scripts/ui-phase5j-ci-per-file-worker-successor-guard.mjs",
  "scripts/ui-phase6a-library-source-successor-guard.mjs",
  "scripts/ui-phase6b-library-shot-bind-successor-guard.mjs",
  "scripts/ui-phase6c-runs-queue-unify-successor-guard.mjs",
  "scripts/ui-phase6d-scale-integrity-successor-guard.mjs",
  "scripts/ui-phase7a-source-ready-successor-guard.mjs",
  "scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs",
  "scripts/ui-phase7b-tsv-preview-successor-guard.mjs",
  "scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs",
  "scripts/workbench-visual-refinement-successor-guard.mjs",
  "src/app/ReadinessLifecycleBoundary.test.ts",
  "src/app/UiPhase5CiRepairBoundary.test.ts",
].sort();
export const PHASE7C_ADDED = [
  ".github/workflows/frontend-only-ci.yml",
  "scripts/ui-phase7c-frontend-ci-budget-successor-guard.mjs",
  "src/app/UiPhase7CCiBudgetSuccessorBoundary.test.ts",
].sort();
export const PHASE7C_CONTRACTS = {
  schemaChanged: false,
  backupFormatChanged: false,
  backendChanged: false,
  runtimeBytesChanged: false,
  historicalProofBytesUnchanged: true,
  readinessBoundaryProbesAtNinetySeconds: true,
  phase5CiRepairProbeBudgetRaised: true,
  gpuExecuted: false,
};
const canonical = value => value.replaceAll("\r\n", "\n");
const sha = value => createHash("sha256").update(canonical(value)).digest("hex");
const relevant = path => path === "vite.config.ts" || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(path);
const git = (root, args) => execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const factsCache = new Map();
function facts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!factsCache.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(path => `${base}:${path}\n`).join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(); let offset = 0;
    for (const path of paths) {
      const end = bytes.indexOf(10, offset);
      const header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("Phase7C immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("Phase7C immutable blob boundary");
      blobs.set(path, bytes.subarray(start, start + size)); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("Phase7C immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, {
      paths,
      raw: path => blobs.get(path),
      read: path => {
        if (!text.has(path)) text.set(path, canonical(blobs.get(path).toString("utf8")));
        return text.get(path);
      },
    });
  }
  return factsCache.get(key);
}
export function phase7CScopeSince(root, base) {
  const tip = facts(root, PHASE7C_PARENT), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(path => !old.paths.includes(path) || tip.read(path) !== old.read(path)),
    ...PHASE7C_CHANGED, ...PHASE7C_ADDED, PHASE7C_PROOF,
  ])].sort();
}
export function phase7CFixtureFiles(root) {
  return [...facts(root, PHASE7C_PARENT).paths, ...PHASE7C_ADDED, PHASE7C_PROOF];
}
function walk(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(entry => {
    const path = `${dir}/${entry.name}`;
    if (entry.isSymbolicLink()) throw Error(`Phase7C symbolic source: ${path}`);
    return entry.isDirectory() ? walk(root, path) : [path];
  }).sort();
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], fail = id => violations.push(`ui-phase7c-frontend-ci-budget-${id}`);
  const parent = facts(workspace, PHASE7C_PARENT);
  // Every predecessor reads through this projection. Read each live file once per
  // validation; the outer cachedBoundary session still re-hashes the live tree
  // afterwards and rejects any drift that happens while validation runs.
  const liveBytes = new Map();
  const live = path => {
    if (override && Object.hasOwn(override, path)) return canonical(override[path]);
    if (!liveBytes.has(path)) liveBytes.set(path, canonical(readFileSync(join(workspace, path), "utf8")));
    return liveBytes.get(path);
  };
  const scope = [...PHASE7C_CHANGED, ...PHASE7C_ADDED].sort();
  const denied = () => ({ violations, changedPaths: [], addedPaths: [], afterHashes: {},
    read: path => canonical(readFileSync(join(workspace, path), "utf8")) });
  let proof;
  try {
    proof = JSON.parse(live(PHASE7C_PROOF));
    const keys = ["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort();
    if (JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(keys)
      || live(PHASE7C_PROOF) !== JSON.stringify(JSON.parse(live(PHASE7C_PROOF)), null, 2) + "\n") fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "UI_PHASE7C_FRONTEND_CI_BUDGET_SUCCESSOR"
      || proof.parentHead !== PHASE7C_PARENT || proof.planningParentHead !== PHASE7C_PARENT) fail("header");
    if (JSON.stringify(proof.contracts) !== JSON.stringify(PHASE7C_CONTRACTS)) fail("contracts");
    if (JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("scope");
    if (git(workspace, ["rev-parse", PHASE7C_PARENT]).trim() !== PHASE7C_PARENT) fail("parent-missing");

    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant).sort();
    const expected = [...new Set([...parent.paths, ...PHASE7C_ADDED, PHASE7C_PROOF])].sort();
    if (JSON.stringify(inventory) !== JSON.stringify(expected)) fail("inventory");
    const actual = ["vite.config.ts", ...[".github/workflows", "src", "scripts", "docs/architecture", "src-tauri"]
      .flatMap(dir => walk(workspace, dir))].filter(relevant).sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) fail("filesystem-inventory");

    const changed = parent.paths.filter(path => live(path) !== parent.read(path)).sort();
    const added = inventory.filter(path => !parent.paths.includes(path)).sort();
    if (JSON.stringify(changed) !== JSON.stringify(PHASE7C_CHANGED)) fail("unapproved-source");
    if (JSON.stringify(added) !== JSON.stringify([...PHASE7C_ADDED, PHASE7C_PROOF].sort())) fail("unapproved-addition");
    if (changed.some(path => path.startsWith("src-tauri/runtime_packages/") || path.startsWith("src-tauri/migrations/"))) fail("runtime-or-migration-drift");

    for (const path of PHASE7C_CHANGED) {
      if (!parent.paths.includes(path) || proof.files?.[path]?.beforeHash !== sha(parent.read(path))
        || proof.files?.[path]?.afterHash !== sha(live(path))) fail(`changed:${path}`);
    }
    for (const path of PHASE7C_ADDED) {
      if (parent.paths.includes(path) || proof.files?.[path]?.beforeHash !== null
        || proof.files?.[path]?.afterHash !== sha(live(path))) fail(`added:${path}`);
    }

    const readiness = live("src/app/ReadinessLifecycleBoundary.test.ts");
    const readinessIts = [...readiness.matchAll(/^[ \t]*it\s*\(/gm)];
    const readinessBudgets = [...readiness.matchAll(/\}\)?\s*,\s*90000\s*\)/g)];
    if (readinessIts.length !== 6 || readinessBudgets.length < readinessIts.length || /360000/.test(readiness)) fail("readiness-timeout-budget");
    const repair = live("src/app/UiPhase5CiRepairBoundary.test.ts");
    if (!/keeps live freshness for frozen proofs, Runtime raw bytes and manifest deletion[\s\S]*?\}\),\s*300000\);/.test(repair)) fail("slow-ci-probe-timeout-budget");

    for (const path of Object.keys(override ?? {}).filter(path => relevant(path) && !scope.includes(path) && path !== PHASE7C_PROOF)) {
      if (!parent.paths.includes(path) || live(path) !== parent.read(path)) fail(`undeclared:${path}`);
    }
  } catch (error) { fail(`validation:${String(error).slice(0, 180)}`); }
  return {
    violations,
    changedPaths: PHASE7C_CHANGED,
    addedPaths: [...PHASE7C_ADDED, PHASE7C_PROOF],
    afterHashes: Object.fromEntries(scope.map(path => [path, proof?.files?.[path]?.afterHash ?? ""])),
    read: path => PHASE7C_CHANGED.includes(path) ? parent.read(path) : live(path),
  };
}
export function uiPhase7CParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "uiPhase7CParentReader", undefined, () => validate(root, undefined), PHASE7C_PROOF);
}
