import type { AssetView } from "./asset";

export const referenceAnchorKinds = ["CHARACTER", "SCENE", "PROP", "STYLE"] as const;

export type ReferenceAnchorKind = (typeof referenceAnchorKinds)[number];

export interface ReferenceAnchorAssetView {
  assetId: string;
  ordinal: number;
  asset?: AssetView | null;
}

export interface ReferenceAnchorView {
  id: string;
  projectId: string;
  kind: ReferenceAnchorKind;
  name: string;
  description: string;
  assets: ReferenceAnchorAssetView[];
  primaryAssetId?: string | null;
  usable: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ReferenceAnchorRequest {
  projectId: string;
  kind: ReferenceAnchorKind;
  name: string;
  description: string;
  assetIds: string[];
}

export interface ReferenceAnchorUpdateRequest extends ReferenceAnchorRequest {
  anchorId: string;
}
