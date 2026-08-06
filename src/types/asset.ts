export interface AssetView {
  id: string;
  category: "source_image" | "generated_image" | string;
  name: string;
  originalName: string;
  mimeType: string;
  width: number;
  height: number;
  fileSize: number;
  createdAt: string;
}

export type AssetCategoryFilter = "ALL" | "SOURCE_IMAGE" | "GENERATED_IMAGE";

export interface AssetLibraryPage {
  items: AssetView[];
  nextCursor?: PageCursor;
}

export interface PageCursor {
  createdAt: string;
  id: string;
}
