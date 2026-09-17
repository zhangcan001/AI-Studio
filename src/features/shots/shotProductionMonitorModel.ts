import type { ProductionBatchArtifactsDto } from "../../types/artifact";

export function safeManifestPart(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "_").replace(/^[._-]+|[._-]+$/g, "") || "batch";
}

export function buildLocalDeliveryManifest(batch: ProductionBatchArtifactsDto, expectedBatchId: string) {
  if (!expectedBatchId || batch.batchId !== expectedBatchId) return undefined;
  const items = [...batch.items]
    .sort((left, right) => left.ordinal - right.ordinal || left.productionItemId.localeCompare(right.productionItemId))
    .map((item) => ({
      productionItemId: item.productionItemId,
      ordinal: item.ordinal,
      productionItemStatus: item.productionItemStatus,
      taskId: item.task?.id,
      artifacts: item.artifacts.map((artifact) => ({
        artifactId: artifact.id,
        name: artifact.name,
        mediaType: artifact.mediaType,
        availability: artifact.availability,
        reviewStatus: artifact.reviewStatus,
      })),
    }));
  return {
    manifestType: "LOCAL_DELIVERY_MANIFEST" as const,
    manifestVersion: 2 as const,
    batchId: batch.batchId,
    batchName: batch.batchName,
    generatedAt: new Date().toISOString(),
    total: batch.total,
    succeeded: batch.succeeded,
    failed: batch.failed,
    items,
  };
}
