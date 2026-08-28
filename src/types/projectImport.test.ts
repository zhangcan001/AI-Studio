import { describe, expect, it } from "vitest";
import {
  buildImportDryRunResult,
  failedImportResult,
  importFormatForFileName,
  importIssueFromError,
  normalizeImportContent,
} from "./projectImport";

const row = (overrides: Partial<Parameters<typeof buildImportDryRunResult>[0]["rows"][number]> = {}) => ({
  rowNumber: 1,
  name: "镜头 01",
  description: "入口",
  imagePrompt: "暗色电影光",
  videoPrompt: "缓慢推进",
  errors: [],
  warnings: [],
  ...overrides,
});

describe("project import dry-run types", () => {
  it("recognizes only the formats backed by the existing importer", () => {
    expect(importFormatForFileName("shots.JSON")).toBe("json");
    expect(importFormatForFileName("shots.tsv")).toBe("tsv");
    expect(importFormatForFileName("shots.txt")).toBe("tsv");
    expect(importFormatForFileName("shots.csv")).toBeUndefined();
  });

  it("normalizes a UTF-8 BOM without changing the imported payload", () => {
    expect(normalizeImportContent("\uFEFF{\"schemaVersion\":1}")).toBe("{\"schemaVersion\":1}");
  });

  it("maps row errors and warnings into readiness without inventing updates", () => {
    const result = buildImportDryRunResult({
      total: 2,
      valid: 1,
      invalid: 1,
      warnings: 1,
      rows: [
        row(),
        row({
          rowNumber: 2,
          name: "重复镜头",
          errors: [{ code: "DUPLICATE_NAME", message: "名称重复", rowNumber: 2, shotId: "shot-2" }],
          warnings: [{ code: "OPTIONAL_FIELD_MISSING", message: "缺少可选字段", rowNumber: 2 }],
        }),
      ],
    });

    expect(result.readiness).toMatchObject({
      ready: false,
      totalRecords: 2,
      validRecords: 1,
      invalidRecords: 1,
      errorCount: 1,
      warningCount: 1,
      blockingCount: 1,
    });
    expect(result.summary).toEqual({});
    expect(result.issues).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: "DUPLICATE_NAME", entityId: "shot-2", row: 2, blocking: true }),
      expect.objectContaining({ code: "OPTIONAL_FIELD_MISSING", blocking: false }),
    ]));
    expect(result.rows).toHaveLength(2);
  });

  it("reports a successful create-only dry-run", () => {
    const result = buildImportDryRunResult({ total: 1, valid: 1, invalid: 0, warnings: 0, rows: [row()] });
    expect(result.readiness.ready).toBe(true);
    expect(result.summary).toEqual({ createCount: 1 });
  });

  it("normalizes backend failures into a blocking issue", () => {
    const issue = importIssueFromError({ code: "BULK_IMPORT_INVALID_JSON", message: "JSON 解析失败" });
    expect(failedImportResult(issue)).toMatchObject({
      readiness: { ready: false, blockingCount: 1, errorCount: 1 },
      issues: [expect.objectContaining({ code: "BULK_IMPORT_INVALID_JSON", blocking: true })],
    });
  });
});
