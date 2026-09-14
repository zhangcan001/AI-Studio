export interface GenerationToolUsageView {
  id: string;
  generationId: string;
  toolInstanceId: string;
  toolVersionId?: string | null;
  metadata: unknown;
  createdAt: string;
}

export interface GenerationAssetVersionView {
  id: string;
  generationId: string;
  outputId: string;
  ordinal: number;
  assetVersionId: string;
  relationType: string;
  createdAt: string;
}
