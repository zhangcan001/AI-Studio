export interface AssetView {
  id: string;
  name: string;
  originalName: string;
  mimeType: string;
  width: number;
  height: number;
  fileSize: number;
  createdAt: string;
  metadata: Record<string, unknown>;
}
