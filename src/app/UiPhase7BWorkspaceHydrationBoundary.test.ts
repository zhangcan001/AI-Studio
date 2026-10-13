// @vitest-environment node
import { it, expect } from "vitest";
import { readFileSync, writeFileSync, copyFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { HYDRATION_PROOF, uiPhase7BWorkspaceHydrationParentReader } from "../../scripts/ui-phase7b-workspace-hydration-successor-guard.mjs";
import { uiPhase7BTsvReceiptParentReader } from "../../scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs";
import { uiPhase7AParentReader } from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";

it("validates hydration before replaying the immutable receipt checkpoint", () => {
  expect(uiPhase7BWorkspaceHydrationParentReader(".").violations).toEqual([]);
  expect(uiPhase7BTsvReceiptParentReader(".").violations).toEqual([]);
}, 120000);

it("rejects live runtime bytes, deleted proof and illegal addition, then accepts restored isolated bytes", () => {
  const fixture = mkdtempSync(join(tmpdir(), "phase7b-hydration-proof-"));
  try {
    const paths: string[] = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], { encoding: "utf8" }).trim().split(/\r?\n/);
    for (const path of paths) {
      mkdirSync(dirname(join(fixture, path)), { recursive: true });
      copyFileSync(path, join(fixture, path));
    }
    execFileSync("git", ["init", "-q", fixture]);
    writeFileSync(join(fixture, ".git/objects/info/alternates"), resolve(".git/objects").replaceAll("\\", "/") + "\n");
    execFileSync("git", ["config", "core.autocrlf", "true"], { cwd: fixture });
    // Match historical boundary fixtures: immutable objects available, no HEAD/index.
    expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations).toEqual([]);
    expect(uiPhase7AParentReader(fixture).violations).toEqual([]);
    const metadata = join(fixture, "src-tauri/.gitignore"), metadataBytes = readFileSync(metadata);
    try {
      writeFileSync(metadata, readFileSync(metadata, "utf8") + "\n# undeclared metadata change\n");
      expect(uiPhase7AParentReader(fixture).violations.length).toBeGreaterThan(0);
    } finally { writeFileSync(metadata, metadataBytes); }
    expect(uiPhase7AParentReader(fixture).violations).toEqual([]);
    const runtime = join(fixture, paths.find((p: string) => p.startsWith("src-tauri/runtime_packages/"))!);
    const bytes = readFileSync(runtime);
    try {
      writeFileSync(runtime, readFileSync(runtime, "utf8") + "\r\n");
      expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations).toContain("ui-phase7b-hydration-runtime-package-bytes");
    } finally { writeFileSync(runtime, bytes); }
    const proof = join(fixture, HYDRATION_PROOF), proofBytes = readFileSync(proof);
    try {
      rmSync(proof);
      expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations.length).toBeGreaterThan(0);
    } finally { writeFileSync(proof, proofBytes); }
    const illegal = join(fixture, "src/unauthorized-hydration.ts");
    try {
      writeFileSync(illegal, "export const bypass = true;\n");
      expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations.length).toBeGreaterThan(0);
    } finally { rmSync(illegal); }
    expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations).toEqual([]);
  } finally { rmSync(fixture, { recursive: true, force: true }); }
}, 120000);

it("rejects forged parent identity, changed source, undeclared files and missing proof", () => {
  const proof = JSON.parse(readFileSync(HYDRATION_PROOF, "utf8"));
  const source = "src/features/shots/ShotWorkspace.tsx";
  const cases = [
    { [HYDRATION_PROOF]: JSON.stringify({ ...proof, parentHead: "forged" }) },
    { [HYDRATION_PROOF]: "" },
    { [source]: readFileSync(source, "utf8") + "\n// unauthorized edit" },
    { "src/features/shots/unauthorized.ts": "export const bypass = true;" },
    { "src/features/shots/ShotBatchAssetBindPanel.tsx": "// removed receipt gate" },
  ];
  for (const override of cases) expect(uiPhase7BWorkspaceHydrationParentReader(".", override).violations.length).toBeGreaterThan(0);
  expect(uiPhase7BWorkspaceHydrationParentReader(".").violations).toEqual([]);
}, 120000);
