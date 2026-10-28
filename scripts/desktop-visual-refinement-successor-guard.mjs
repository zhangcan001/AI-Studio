import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead } from "./boundary-validation-cache.mjs";

import { workbenchVisualParentReader, workbenchVisualFixtureFiles, workbenchVisualScopeSince, WORKBENCH_VISUAL_BASE } from './workbench-visual-refinement-successor-guard.mjs';

export const DESKTOP_VISUAL_BASE = "0c8a974d32cfa7e671e9b1b2e6088a61370cabf1";
export const DESKTOP_VISUAL_PROOF = "docs/architecture/desktop-visual-refinement-successor.json";
export const DESKTOP_VISUAL_CHANGED = [
  "scripts/h3-standard-reference-successor-guard.mjs",
  "scripts/style-boundary-guard.mjs",
  "src/styles/studioQuality.css",
  "src/styles/studioTokens.css"
].sort();
// Presentation-only checkpoint: no added Runtime Package paths.
export const DESKTOP_VISUAL_RUNTIME = [];
export const DESKTOP_VISUAL_ADDED = [
  "scripts/desktop-visual-refinement-successor-guard.mjs",
  "scripts/testing/desktop-visual-refinement.test.mjs",
  "src/styles/StudioVisualRefinement.test.ts"
].sort();
export const DESKTOP_VISUAL_CONTRACTS = {
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
export function desktopVisualScopeSince(root, base) {
  const tip = facts(root, DESKTOP_VISUAL_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...DESKTOP_VISUAL_CHANGED, ...DESKTOP_VISUAL_ADDED, DESKTOP_VISUAL_PROOF,
    ...workbenchVisualScopeSince(root, WORKBENCH_VISUAL_BASE),
  ])].sort();
}
export function desktopVisualFixtureFiles(root) {
  return [...new Set([...facts(root, DESKTOP_VISUAL_BASE).paths, ...DESKTOP_VISUAL_ADDED, DESKTOP_VISUAL_PROOF, ...workbenchVisualFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, DESKTOP_VISUAL_BASE);
  const fail = id => violations.push("desktop-visual-refinement-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(DESKTOP_VISUAL_PROOF));
    const scope = [...DESKTOP_VISUAL_CHANGED, ...DESKTOP_VISUAL_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "DESKTOP_VISUAL_REFINEMENT"
        || proof.parentHead !== DESKTOP_VISUAL_BASE || proof.planningParentHead !== DESKTOP_VISUAL_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(DESKTOP_VISUAL_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", DESKTOP_VISUAL_BASE]).trim() !== DESKTOP_VISUAL_BASE) fail("ancestor");
    const allowed = [...scope, DESKTOP_VISUAL_PROOF].sort();
    const expected = [...parent.paths, ...DESKTOP_VISUAL_ADDED, DESKTOP_VISUAL_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !successor.addedPaths.includes(p) && !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of DESKTOP_VISUAL_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of DESKTOP_VISUAL_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of DESKTOP_VISUAL_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...DESKTOP_VISUAL_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...DESKTOP_VISUAL_ADDED, DESKTOP_VISUAL_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...DESKTOP_VISUAL_CHANGED, ...DESKTOP_VISUAL_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => DESKTOP_VISUAL_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function desktopVisualParentReader(root, override = undefined) {
  const successor = workbenchVisualParentReader(root);
  const check = () => {
    const historical = validate(root, override, successor);
    return { ...historical, violations: [...successor.violations, ...historical.violations] };
  };
  if (override) return check();
  return cachedBoundary(root, "desktopVisualParentReader", undefined, check, DESKTOP_VISUAL_PROOF);
}
