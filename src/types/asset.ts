export interface AssetView {
  id: string;
  assetType?: "image" | "video" | string;
  category: "source_image" | "generated_image" | "generated_video" | string;
  name: string;
  originalName: string;
  mimeType: string;
  width?: number;
  height?: number;
  durationMs?: number | null;
  fileSize: number;
  createdAt: string;
  thumbnailAvailable?: boolean;
}

export type AssetCategoryFilter = "ALL" | "SOURCE_IMAGE" | "GENERATED_IMAGE" | "GENERATED_VIDEO";

export interface AssetLibraryPage {
  items: AssetView[];
  nextCursor?: PageCursor;
}

export interface PageCursor {
  createdAt: string;
  id: string;
}
