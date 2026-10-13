// Validate the new hydration checkpoint before projecting immutable receipt-stage bytes.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from "./boundary-validation-cache.mjs";
import { uiPhase7BBackupPromptParentReader, phase7BBackupPromptScopeSince, phase7BBackupPromptFixtureFiles } from "./ui-phase7b-backup-prompt-successor-guard.mjs";

export const HYDRATION_BASE = "0b0bd802aa185f4bc76409808cdae31d65aad93d";
export const HYDRATION_PROOF = "docs/architecture/ui-phase7b-workspace-hydration-successor.json";
export const HYDRATION_CHANGED = [
  "scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs",
  "scripts/ui-phase7b-tsv-preview-successor-guard.mjs",
  "scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs",
  "scripts/ui-phase7a-source-ready-successor-guard.mjs",
  "scripts/boundary-validation-cache.mjs",
  "src/features/shots/ShotWorkspace.tsx",
  "src/features/shots/ShotWorkspace.production.test.tsx",
  "src/app/UiPhase6ALibrarySourceBoundary.test.ts",
  "src/features/production/ProductionPackageWorkspace.test.tsx",
  "vite.config.ts",
].sort();
export const HYDRATION_ADDED = [
  "scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.d.mts",
  "scripts/ui-phase7b-workspace-hydration-successor-guard.mjs",
  "src/app/UiPhase7BWorkspaceHydrationBoundary.test.ts",
  "scripts/testing/boundary-rpc-yield.mjs",
].sort();
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const git = (root, args, encoding = "utf8") => execFileSync("git", args, { cwd: root, encoding, maxBuffer: 64 * 1024 * 1024 });
const relevant = p => p === "vite.config.ts" || /^(src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const parents = new Map();
function parentFacts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!parents.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(p => base + ":" + p + "\n").join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, length = Number(header[2]);
      if (bytes[start + length] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, bytes.subarray(start, start + length)); offset = start + length + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const texts = new Map();
    parents.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!texts.has(p)) texts.set(p, canonical(blobs.get(p).toString("utf8")));
      return texts.get(p);
    } });
  }
  return parents.get(key);
}
// Historical fixtures intentionally have an empty Git index. Scope is established
// by the validated live-byte checkpoint, NOT git diff's index-dependent deletions.
export function phase7BHydrationScopeSince(root, base) {
  return phase7BBackupPromptScopeSince(root, base);
}
export function phase7BHydrationFixtureFiles(root) {
  return phase7BBackupPromptFixtureFiles(root);
}
const contracts = { historicalProofBytesUnchanged: true, runtimeBytesUnchanged: true,
  migrationUnchanged: true, backupUnchanged: true, singleQueue: true,
  shotQueryAuthoritative: true, ancillaryFailureVisible: true, gpuExecuted: false };
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [];
  const fail = id => violations.push("ui-phase7b-hydration-" + id);
  const live = p => canonical(successor.read(p));
  const facts = parentFacts(workspace, HYDRATION_BASE);
  const parent = p => facts.read(p);
  let proof;
  try {
    proof = JSON.parse(live(HYDRATION_PROOF));
    const scope = [...HYDRATION_CHANGED, ...HYDRATION_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "UI_PHASE7B_WORKSPACE_HYDRATION"
        || proof.parentHead !== HYDRATION_BASE || proof.planningParentHead !== HYDRATION_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(contracts)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", HYDRATION_BASE]).trim() !== HYDRATION_BASE) fail("ancestor");
    const allowed = [...scope, HYDRATION_PROOF].sort();
    const basePaths = facts.paths;
    const actualPaths = ["src", "src-tauri", "scripts", "docs/architecture"].flatMap(d => {
      // src-tauri contains ignored build output; Git inventory below covers those source paths.
      return d === "src-tauri" ? [] : files(workspace, d);
    }).filter(p => !p.includes("node_modules/"));
    const gitPaths = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"]).trim().split(/\r?\n/)
      .filter(p => relevant(p) && !successor.addedPaths.includes(p));
    const expectedPaths = [...basePaths, ...HYDRATION_ADDED, HYDRATION_PROOF].sort();
    if (JSON.stringify([...new Set(gitPaths)].sort()) !== JSON.stringify(expectedPaths)) fail("inventory");
    if (actualPaths.some(p => !successor.addedPaths.includes(p) && !expectedPaths.includes(p))) fail("illegal-file");
    const changed = basePaths.filter(p => live(p) !== parent(p));
    const added = gitPaths.filter(p => !basePaths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    for (const p of HYDRATION_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of HYDRATION_ADDED) {
      if (basePaths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Compare undeclared tracked changes to immutable parent, including override negative probes.
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!basePaths.includes(p) || live(p) !== parent(p)) fail("undeclared:" + p);
    }
    const runtime = basePaths.filter(p => p.startsWith("src-tauri/runtime_packages/"));
    const rawAggregate = read => sha(JSON.stringify(runtime.map(p => [p, sha(read(p))])));
    const baselines = JSON.parse(parent("docs/architecture/minimax-video-phase2.json")).runtimeByteBaselines;
    const profile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? "git";
    if (rawAggregate(p => facts.raw(p)) !== baselines.git
        || rawAggregate(p => readFileSync(join(workspace, p))) !== baselines[profile]) fail("runtime-package-bytes");
    const source = live("src/features/shots/ShotWorkspace.tsx");
    if (!source.includes("await Promise.allSettled([") || !source.includes('shotsResult.status === "rejected"')
        || !source.includes("generation !== reloadGeneration.current")
        || !source.includes("setShots(nextShots)") || !source.includes("setLoadWarning(")) fail("hydration-contract");
  } catch (error) { fail("validation:" + String(error).slice(0, 160)); }
  return { violations: [...successor.violations, ...violations],
    changedPaths: [...new Set([...HYDRATION_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...HYDRATION_ADDED, HYDRATION_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...HYDRATION_CHANGED, ...HYDRATION_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => HYDRATION_CHANGED.includes(p) ? parent(p) : live(p) };
}
export function uiPhase7BWorkspaceHydrationParentReader(root, override = undefined) {
  const successor = uiPhase7BBackupPromptParentReader(root, override);
  if (successor.violations.length) {
    const violations = [...successor.violations];
    // Preserve the historical guard's precise public rejection reason.
    if (violations.includes("ui-phase7b-backup-prompts-runtime-package-bytes")) violations.push("ui-phase7b-hydration-runtime-package-bytes");
    return { ...successor, violations };
  }
  // These overrides are source bytes, not cachedBoundary's immutable review object.
  if (override) return validate(root, override, successor);
  return cachedBoundary(root, "uiPhase7BWorkspaceHydrationParentReader", override,
    () => validate(root, override, successor), HYDRATION_PROOF);
}
