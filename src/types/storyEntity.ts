export type StoryEntityKind = "character" | "scene" | "prop";

export interface StoryEntity {
  id: string;
  projectId: string;
  kind: StoryEntityKind;
  name: string;
  description: string;
  coverAssetId: string | null;
  imageAssetIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface StoryEntityUpsert {
  name: string;
  description: string;
  coverAssetId?: string | null;
  imageAssetIds: string[];
}
