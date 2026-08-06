export interface AssetView {
  id: string;
  assetType?: "image" | "video" | "audio" | string;
  category: "source_image" | "source_video" | "source_audio" | "generated_image" | "generated_video" | string;
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

export type AssetCategoryFilter =
  | "ALL"
  | "SOURCE_IMAGE"
  | "SOURCE_VIDEO"
  | "SOURCE_AUDIO"
  | "GENERATED_IMAGE"
  | "GENERATED_VIDEO";

export interface AssetLibraryPage {
  items: AssetView[];
  nextCursor?: PageCursor;
}

export interface PageCursor {
  createdAt: string;
  id: string;
}
