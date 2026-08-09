export function buildAssetMediaUrl(
  projectId: string,
  assetId: string,
  mediaKind: "video" | "audio" = "video",
  userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent,
): string {
  const origin = /Windows/i.test(userAgent)
    ? "http://aistudio-media.localhost"
    : "aistudio-media://localhost";
  return `${origin}/${mediaKind}?projectId=${encodeURIComponent(projectId)}&assetId=${encodeURIComponent(assetId)}`;
}
