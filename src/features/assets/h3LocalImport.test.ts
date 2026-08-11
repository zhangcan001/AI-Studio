import { describe, expect, it } from "vitest";
import { formatPromptBytes, localImportCanCommit, localImportModeLabel, localImportStatusLabel } from "./h3LocalImport";
import type { H3LocalImportInspection } from "../../types/h3LocalImport";

function inspection(overrides: Partial<H3LocalImportInspection> = {}): H3LocalImportInspection {
  return {
    sessionId: "h3_local_test",
    displayRootName: "fixture",
    mode: "PAIRING",
    detectedManifest: false,
    imageCount: 2,
    promptCount: 2,
    readyCount: 2,
    errorCount: 0,
    pairs: [],
    errors: [],
    warnings: [],
    ...overrides,
  };
}

describe("H3 local import UI policy", () => {
  it("labels both source modes and pair statuses", () => {
    expect(localImportModeLabel("PAIRING")).toBe("自动同名配对");
    expect(localImportModeLabel("MANIFEST")).toBe("JSON 批量清单");
    expect(localImportStatusLabel("READY")).toBe("可生成");
    expect(localImportStatusLabel("MISSING_PROMPT")).toBe("缺少 Prompt");
    expect(localImportStatusLabel("INVALID_PATH")).toBe("路径不安全");
  });

  it("requires a clean inspection, runtime readiness, and free admission", () => {
    expect(localImportCanCommit(inspection(), true, false)).toBe(true);
    expect(localImportCanCommit(inspection({ errorCount: 1 }), true, false)).toBe(false);
    expect(localImportCanCommit(inspection({ readyCount: 0 }), true, false)).toBe(false);
    expect(localImportCanCommit(inspection(), false, false)).toBe(false);
    expect(localImportCanCommit(inspection(), true, true)).toBe(false);
    expect(localImportCanCommit(inspection({ readyCount: 101 }), true, false)).toBe(false);
  });

  it("formats prompt bytes without exposing a local path", () => {
    expect(formatPromptBytes(2048)).toMatch(/2,048 B|2 048 B|2048 B/);
    expect(formatPromptBytes(undefined)).toBe("—");
  });
});
