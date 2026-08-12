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
  sourceTaskId?: string;
  thumbnailAvailable?: boolean;
  isFavorite: boolean;
  tags: Array<{ id: string; name: string }>;
}

export interface AssetImportFailure {
  displayName: string;
  error: string;
}

export interface AssetSourceImportBatch {
  imported: AssetView[];
  failed: AssetImportFailure[];
  cancelled: boolean;
}

export type AssetCategoryFilter =
  | "ALL"
  | "SOURCE_IMAGE"
  | "SOURCE_VIDEO"
  | "SOURCE_AUDIO"
  | "GENERATED_IMAGE"
  | "GENERATED_VIDEO";

export type AssetMediaTypeFilter = "ALL" | "IMAGE" | "VIDEO" | "AUDIO";
export type AssetSourceFilter = "ALL" | "SOURCE" | "GENERATED";
export type AssetCreatedOrder = "NEWEST" | "OLDEST";

export interface AssetLibraryQuery {
  projectId: string;
  category: AssetCategoryFilter;
  keyword?: string;
  mediaType: AssetMediaTypeFilter;
  sourceKind: AssetSourceFilter;
  favoriteOnly?: boolean;
  tagId?: string;
  createdOrder: AssetCreatedOrder;
  cursor?: PageCursor;
  limit?: number;
}

export interface AssetLibraryPage {
  items: AssetView[];
  nextCursor?: PageCursor;
}

export interface AssetDeleteInspectionItem {
  assetId: string;
  name: string;
  assetType: string;
  fileSize: number;
  canDelete: boolean;
  blockingReasons: string[];
  warnings: string[];
}

export interface AssetDeleteInspection {
  items: AssetDeleteInspectionItem[];
  deletable: string[];
  blocked: string[];
  historicalReferences: string[];
}

export interface AssetDeleteResult {
  deletedCount: number;
  warnings: string[];
}

export interface PageCursor {
  createdAt: string;
  id: string;
}
