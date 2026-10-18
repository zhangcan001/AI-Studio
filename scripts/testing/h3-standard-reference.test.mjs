// @vitest-environment node
import { it, expect } from "vitest";
import { readFileSync, writeFileSync, copyFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { STANDARD_REFERENCE_BASE, STANDARD_REFERENCE_PROOF, STANDARD_REFERENCE_RUNTIME, standardReferenceParentReader } from "../h3-standard-reference-successor-guard.mjs";
import { R1_METADATA_PROOF, uiPhase7BR1MetadataParentReader } from "../ui-phase7b-r1-metadata-successor-guard.mjs";

it("adds only the approved standard package, projects parent bytes and preserves historical proof", () => {
  const view = standardReferenceParentReader(".");
  expect(view.violations).toEqual([]);
  const p = "src-tauri/src/application/builtin_runtime_packages.rs";
  expect(view.read(p)).toBe(execFileSync("git", ["show", STANDARD_REFERENCE_BASE + ":" + p], { encoding: "utf8" }));
  expect(readFileSync(R1_METADATA_PROOF)).toEqual(execFileSync("git", ["show", STANDARD_REFERENCE_BASE + ":" + R1_METADATA_PROOF]));
  expect(uiPhase7BR1MetadataParentReader(".").violations).toEqual([]);
}, 120000);

it("rejects forged proof, changed embedded package bytes and undeclared changes", () => {
  const proof = JSON.parse(readFileSync(STANDARD_REFERENCE_PROOF, "utf8"));
  const p = "src-tauri/src/application/builtin_runtime_packages.rs";
  const cases = [
    { [STANDARD_REFERENCE_PROOF]: JSON.stringify({ ...proof, parentHead: "forged" }) },
    { [STANDARD_REFERENCE_PROOF]: JSON.stringify({ ...proof, contracts: { ...proof.contracts, turboExposed: true } }) },
    { [STANDARD_REFERENCE_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [p]: { ...proof.files[p], afterHash: "0".repeat(64) } } }) },
    { [p]: readFileSync(p, "utf8") + "\n// undeclared execution change\n" },
    { ["src-tauri/src/application/shot_video_input_service.rs"]: "forged entity retention" },
    { ["src-tauri/src/application/production_queue_service.rs"]: "forged queue" },
    { [R1_METADATA_PROOF]: "forged history" },
    ...STANDARD_REFERENCE_RUNTIME.map(p => ({ [p]: readFileSync(p, "utf8").replaceAll("\n", "\r\n") })),
  ];
  for (const changed of cases) expect(standardReferenceParentReader(".", changed).violations.length).toBeGreaterThan(0);
  expect(standardReferenceParentReader(".").violations).toEqual([]);
}, 120000);

it("invalidates cached fixtures on mutation, removal and illegal additions; restoring bytes passes", () => {
  const root = mkdtempSync(join(tmpdir(), "h3-standard-proof-"));
  try {
    const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], { encoding: "utf8" }).trim().split(/\r?\n/);
    for (const p of paths) { mkdirSync(dirname(join(root, p)), { recursive: true }); copyFileSync(p, join(root, p)); }
    execFileSync("git", ["init", "-q", root]);
    writeFileSync(join(root, ".git/objects/info/alternates"), resolve(".git/objects").replaceAll("\\", "/") + "\n");
    const validate = () => standardReferenceParentReader(root).violations;
    expect(validate()).toEqual([]);
    for (const p of [STANDARD_REFERENCE_PROOF, ...STANDARD_REFERENCE_RUNTIME, "src-tauri/src/application/builtin_runtime_packages.rs", "src-tauri/src/application/shot_video_input_service.rs"]) {
      const path = join(root, p), bytes = readFileSync(path);
      try {
        writeFileSync(path, Buffer.concat([bytes, Buffer.from("\ninvalid mutation\n")]));
        expect(validate().length).toBeGreaterThan(0);
        rmSync(path);
        expect(validate().length).toBeGreaterThan(0);
      } finally { writeFileSync(path, bytes); }
      expect(validate()).toEqual([]);
    }
    for (const p of ["src/illegal-standard.ts", "src-tauri/runtime_packages/illegal.json", ".github/workflows/illegal.yml"]) {
      const path = join(root, p);
      try { writeFileSync(path, "forged"); expect(validate().length).toBeGreaterThan(0); }
      finally { rmSync(path); }
      expect(validate()).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 120000);
