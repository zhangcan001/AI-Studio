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

export interface StoryEntityImageImportAsset {
  id: string;
  name: string;
  originalName: string;
}

export interface StoryEntityImageImportBatch {
  imported: StoryEntityImageImportAsset[];
  failed: Array<{ displayName: string; error: string }>;
  cancelled: boolean;
}

/** Window event fired after a story entity is created, updated or deleted. */
export const STORY_ENTITIES_CHANGED = "ai-studio:story-entities-changed";

/** Prefer the formal cover; otherwise the first linked image so Library entities without an explicit cover still show an avatar. */
export function entityAvatarAssetId(entity: Pick<StoryEntity, "coverAssetId" | "imageAssetIds">): string | null {
  return entity.coverAssetId ?? entity.imageAssetIds[0] ?? null;
}
