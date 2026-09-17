import { describe, expect, it } from "vitest";
import type { ArtifactDto, ProductionBatchArtifactsDto } from "../../types/artifact";
import { buildLocalDeliveryManifest, safeManifestPart } from "./shotProductionMonitorModel";

function artifact(id: string): ArtifactDto {
  return {
    id,
    taskId: "task-1",
    outputId: "output-1",
    ordinal: 0,
    mediaType: "video",
    name: `${id}.mp4`,
    mimeType: "video/mp4",
    sizeBytes: 1024,
    version: 1,
    createdAt: "2026-09-17T00:00:00Z",
    thumbnailAvailable: false,
    availability: "available",
    reviewStatus: "PENDING",
  };
}

function batch(): ProductionBatchArtifactsDto {
  return {
    batchId: "batch-a",
    batchName: "Batch A",
    status: "COMPLETED",
    total: 2,
    pending: 0,
    running: 0,
    succeeded: 2,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [
      { productionItemId: "item-b", ordinal: 2, productionItemStatus: "SUCCEEDED", artifacts: [artifact("asset-b")] },
      { productionItemId: "item-a", ordinal: 1, productionItemStatus: "SUCCEEDED", artifacts: [artifact("asset-a")] },
    ],
  };
}

describe("shotProductionMonitorModel", () => {
  it("builds a typed delivery manifest from the batch's exact Artifact DTOs", () => {
    const manifest = buildLocalDeliveryManifest(batch(), "batch-a");
    expect(manifest).toMatchObject({
      manifestType: "LOCAL_DELIVERY_MANIFEST",
      manifestVersion: 2,
      batchId: "batch-a",
      items: [
        { productionItemId: "item-a", artifacts: [{ artifactId: "asset-a", availability: "available", reviewStatus: "PENDING" }] },
        { productionItemId: "item-b", artifacts: [{ artifactId: "asset-b" }] },
      ],
    });
    expect(JSON.stringify(manifest)).not.toContain("path");
  });

  it("rejects a manifest when the selected batch identity differs", () => {
    expect(buildLocalDeliveryManifest(batch(), "batch-b")).toBeUndefined();
  });

  it("sanitizes filesystem-independent manifest filename parts", () => {
    expect(safeManifestPart("../batch:/unsafe")).toBe("batch_unsafe");
    expect(safeManifestPart("...")).toBe("batch");
  });
});
