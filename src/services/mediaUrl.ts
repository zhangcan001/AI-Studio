export function buildAssetMediaUrl(projectId: string, assetId: string): string {
  return `aistudio-media://localhost/video?projectId=${encodeURIComponent(projectId)}&assetId=${encodeURIComponent(assetId)}`;
}
