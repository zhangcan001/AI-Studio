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
  metadata: Record<string, unknown>;
}
