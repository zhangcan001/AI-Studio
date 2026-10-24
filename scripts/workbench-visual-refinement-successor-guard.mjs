import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, sessionGit } from "./boundary-validation-cache.mjs";
import { uiPhase7CParentReader, phase7CScopeSince, phase7CFixtureFiles } from "./ui-phase7c-frontend-ci-budget-successor-guard.mjs";

export const WORKBENCH_VISUAL_BASE = "1ed43f27abcca2ae6ac42414bd0deccd46be8ae2";
export const WORKBENCH_VISUAL_PROOF = "docs/architecture/workbench-visual-refinement-successor.json";
export const WORKBENCH_VISUAL_CHANGED = [
  "scripts/desktop-visual-refinement-successor-guard.mjs",
  "scripts/testing/ui-phase7b-r1-metadata.test.mjs",
  "src/features/create/CreatePage.css",
  "src/features/runs/RunsPage.css"
].sort();
// Presentation-only checkpoint: no added Runtime Package paths.
export const WORKBENCH_VISUAL_RUNTIME = [];
export const WORKBENCH_VISUAL_ADDED = [
  "scripts/testing/workbench-visual-refinement.test.mjs",
  "scripts/workbench-visual-refinement-successor-guard.mjs",
  "src/styles/StudioWorkbenchRefinement.test.ts"
].sort();
export const WORKBENCH_VISUAL_CONTRACTS = {
  schemaChanged: false, backupFormatChanged: false, runtimeBytesChanged: false,
  historicalProofBytesUnchanged: true, presentationOnly: true, gpuExecuted: false,
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
export function workbenchVisualScopeSince(root, base) {
  const tip = facts(root, WORKBENCH_VISUAL_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...WORKBENCH_VISUAL_CHANGED, ...WORKBENCH_VISUAL_ADDED, WORKBENCH_VISUAL_PROOF,
    ...phase7CScopeSince(root, WORKBENCH_VISUAL_BASE),
  ])].sort();
}
export function workbenchVisualFixtureFiles(root) {
  return [...new Set([...facts(root, WORKBENCH_VISUAL_BASE).paths, ...WORKBENCH_VISUAL_ADDED, WORKBENCH_VISUAL_PROOF, ...phase7CFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), successor = uiPhase7CParentReader(workspace, override);
  const violations = [...successor.violations], parent = facts(workspace, WORKBENCH_VISUAL_BASE);
  const fail = id => violations.push("workbench-visual-refinement-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(WORKBENCH_VISUAL_PROOF));
    const scope = [...WORKBENCH_VISUAL_CHANGED, ...WORKBENCH_VISUAL_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "WORKBENCH_VISUAL_REFINEMENT"
        || proof.parentHead !== WORKBENCH_VISUAL_BASE || proof.planningParentHead !== WORKBENCH_VISUAL_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(WORKBENCH_VISUAL_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", WORKBENCH_VISUAL_BASE]).trim() !== WORKBENCH_VISUAL_BASE) fail("ancestor");
    const allowed = [...scope, WORKBENCH_VISUAL_PROOF, ...successor.addedPaths].sort();
    const expected = [...new Set([...parent.paths, ...WORKBENCH_VISUAL_ADDED, WORKBENCH_VISUAL_PROOF, ...successor.addedPaths])].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of WORKBENCH_VISUAL_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of WORKBENCH_VISUAL_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of WORKBENCH_VISUAL_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...WORKBENCH_VISUAL_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...WORKBENCH_VISUAL_ADDED, WORKBENCH_VISUAL_PROOF, ...successor.addedPaths],
    afterHashes: { ...Object.fromEntries([...WORKBENCH_VISUAL_CHANGED, ...WORKBENCH_VISUAL_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => WORKBENCH_VISUAL_CHANGED.includes(p) ? parent.read(p) : successor.read(p) };
}
export function workbenchVisualParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "workbenchVisualParentReader", undefined,
    () => validate(root, undefined), WORKBENCH_VISUAL_PROOF);
}
