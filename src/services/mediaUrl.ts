export function buildAssetMediaUrl(
  projectId: string,
  assetId: string,
  mediaKind: "video" | "audio" = "video",
): string {
  return `aistudio-media://localhost/${mediaKind}?projectId=${encodeURIComponent(projectId)}&assetId=${encodeURIComponent(assetId)}`;
}
