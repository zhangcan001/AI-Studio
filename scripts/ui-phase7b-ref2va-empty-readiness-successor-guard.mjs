// Phase 7B REF2VA admission repair: exact-base successor. Historical proofs stay frozen.
import { createHash } from "node:crypto";
import { uiPhase7BTsvParentReader } from "./ui-phase7b-tsv-preview-successor-guard.mjs";
import { phase7BHydrationScopeSince } from "./ui-phase7b-workspace-hydration-successor-guard.mjs";
import { execFileSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";

export const PHASE7B_BASE = "6f5c3c33860749ccf25aff3d079b9982492fca37";
export const PHASE7B_PROOF = "docs/architecture/ui-phase7b-ref2va-empty-readiness-successor.json";
export const PHASE7B_CHANGED = [
  "src-tauri/src/application/builtin_runtime_packages.rs",
  "src-tauri/src/application/product/h3_resolution.rs",
  "scripts/ui-phase7a-source-ready-successor-guard.mjs",
].sort();
export const PHASE7B_ADDED = [
  "scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs",
  "src/app/UiPhase7BRef2VAEmptyReadinessBoundary.test.ts",
].sort();
export const PHASE7B_ALLOWED = [...PHASE7B_CHANGED, ...PHASE7B_ADDED, PHASE7B_PROOF].sort();

const canonical = value => value.replaceAll("\r\n", "\n");
const sha = value => createHash("sha256").update(canonical(value)).digest("hex");
function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
}
function baseline(root, file) {
  return canonical(git(root, ["show", PHASE7B_BASE + ":" + file]));
}

export function uiPhase7BParentReader(root, override = undefined) {
  const base = resolve(root);
  const successor = uiPhase7BTsvParentReader(root);
  const violations = [...successor.violations];
  const fail = value => violations.push("ui-phase7b-ref2va-" + value);
  let proof;
  try {
    proof = JSON.parse(readFileSync(join(base, PHASE7B_PROOF), "utf8"));
  } catch { fail("missing-manifest"); }
  const current = path => canonical(
    override && Object.hasOwn(override, path)
      ? override[path]
      : successor.read(path)
  );
  const parent = path => baseline(base, path);
  const expectedScope = [...PHASE7B_CHANGED, ...PHASE7B_ADDED].sort();
  if (!proof || proof.schemaVersion !== 1
      || proof.checkpoint !== "UI_PHASE7B_REF2VA_EMPTY_READINESS"
      || proof.parentHead !== PHASE7B_BASE
      || JSON.stringify(Object.keys(proof.files || {}).sort()) !== JSON.stringify(expectedScope))
    fail("proof-scope");
  try {
    const recorded = git(base, ["rev-parse", PHASE7B_BASE]).trim();
    if (recorded !== PHASE7B_BASE) fail("parent-identity");
    const actual = phase7BHydrationScopeSince(base, PHASE7B_BASE);
    const accepted = [...new Set([...PHASE7B_ALLOWED, ...successor.changedPaths, ...successor.addedPaths])].sort();
    if (JSON.stringify(actual) !== JSON.stringify(accepted)) fail("unapproved-tree-change");
    for (const path of PHASE7B_CHANGED) {
      if (!existsSync(join(base, path)) || proof.files[path]?.beforeHash !== sha(parent(path))
          || proof.files[path]?.afterHash !== sha(current(path))) fail("changed:" + path);
    }
    for (const path of PHASE7B_ADDED) {
      if (!existsSync(join(base, path)) || proof.files[path]?.beforeHash !== null
          || proof.files[path]?.afterHash !== sha(current(path))) fail("added:" + path);
    }
    if (!current("src-tauri/src/application/product/h3_resolution.rs").includes("if images + videos == 0"))
      fail("minimum-media-gate");
    if (!current("src-tauri/src/application/builtin_runtime_packages.rs").includes("InputValue::Videos(vec![\"reference.mp4\".into()])"))
      fail("builtin-compile-fixture");
  } catch (error) { fail("verification:" + String(error).slice(0, 120)); }
  return {
    violations,
    addedPaths: [...PHASE7B_ADDED, PHASE7B_PROOF, ...successor.addedPaths],
    afterHashes: { ...Object.fromEntries(expectedScope.map(path => [path, proof?.files?.[path]?.afterHash ?? ""])), ...successor.afterHashes },
    read: path => PHASE7B_CHANGED.includes(path) ? parent(path) : current(path),
  };
}
