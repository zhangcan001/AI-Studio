import type { AssetView, PageCursor } from "../types/asset";
import type { PromptEntryView, PromptVersionView } from "../types/prompt";
import type { ProfileType, ReferenceSetPurpose } from "../types/consistency";

export type ResourceRef =
  | { kind: "asset"; id: string }
  | { kind: "prompt"; id: string }
  | { kind: "profile"; id: string }
  | { kind: "reference-set"; id: string };
export type LibraryCategory = "all" | "media" | "images" | "videos" | "audio" | "prompts" | "profiles" | "reference-sets";
export interface LibraryCursor { projectId: string; category: LibraryCategory; keyword: string | null; position: PageCursor }
export interface LibraryQuery { category: LibraryCategory; keyword: string | null; cursor: LibraryCursor | null; limit: number | null }
export interface LibraryItem { resourceRef: ResourceRef; title: string; subtype: string; createdAt: string; updatedAt: string; thumbnailAvailable: boolean }
export interface LibraryList { items: LibraryItem[]; nextCursor: LibraryCursor | null; coverage: "recent-summary" | "keyset-page" | "complete-category"; coverageMessage: string }

// Profiles retain their typed domain configuration, not a prompt-only projection.
interface ProfileBase { id: string; project_id: string; name: string; active_revision_id: string | null; created_at: string; updated_at: string }
interface CharacterProfile extends ProfileBase { description: string; canonical_prompt: string; negative_prompt: string; default_style_profile_id: string | null; default_reference_set_id: string | null; metadata_json: string }
interface SceneProfile extends ProfileBase { description: string; environment_prompt: string; lighting_prompt: string | null; negative_prompt: string | null; default_style_profile_id: string | null; default_reference_set_id: string | null }
interface PropProfile extends ProfileBase { description: string; canonical_prompt: string; material_prompt: string | null; scale_prompt: string | null; default_reference_set_id: string | null }
interface StyleProfile extends ProfileBase { style_prompt: string; color_prompt: string | null; line_prompt: string | null; negative_prompt: string | null; output_notes: string | null }
export type LibraryProfile = { Character: CharacterProfile } | { Scene: SceneProfile } | { Prop: PropProfile } | { Style: StyleProfile };
export interface LibraryReferenceMember { assetId: string; ordinal: number; role: string | null; isPrimary: boolean; assetName: string; thumbnailAvailable: boolean; width: number; height: number }
export interface LibraryReferenceSet { id: string; name: string; description: string; purpose: ReferenceSetPurpose; ownerProfileType: ProfileType | null; ownerProfileId: string | null; activeRevisionId: string | null; items: LibraryReferenceMember[] }
export type LibraryDetail =
  | { kind: "asset"; asset: AssetView }
  | { kind: "prompt"; prompt: PromptEntryView }
  | { kind: "profile"; profile: LibraryProfile }
  | { kind: "reference-set"; referenceSet: LibraryReferenceSet };

export type LibraryRelationKind = "SHOT_REFERENCE" | "SHOT_SELECTED_RESULT" | "GENERATION_SNAPSHOT_INPUT" | "GENERATION_OUTPUT" | "RUN_RESULT" | "REFERENCE_SET_MEMBER" | "PROFILE_RELATION" | "ACTIVE_TASK" | "ACTIVE_PRODUCTION" | "LEGACY_REVIEW_HISTORY" | "ARTIFACT_REVIEW_PLACEHOLDER" | "ARTIFACT_REVIEW_MEANINGFUL" | "GENERATION_ASSET_VERSION_LINEAGE" | "PROFILE_USAGE" | "REFERENCE_SET_USAGE" | "PROMPT_VERSION_USAGE";
export type LibraryRelationLocation = { kind: "shot"; id: string; stage: string | null } | { kind: "run"; runRef: import("./types").RunRef } | { kind: "resource"; resource: ResourceRef };
export interface LibraryRelation { kind: LibraryRelationKind; title: string; description: string; blocking: boolean; location: LibraryRelationLocation | null }
export interface LibraryDeletionInspection { allowed: boolean; blockers: string[]; warnings: string[]; relations: LibraryRelation[]; consequences: string[] }
export type LibraryVersions = { kind: "asset"; versions: { id: string; versionNumber: number; createdAt: string }[] } | { kind: "prompt"; versions: PromptVersionView[] } | { kind: "unsupported"; reason: string };
export type LibraryCreateIntent =
  | { kind: "asset"; projectId: string; assetId: string; mediaKind: "image" | "video" | "audio" }
  | { kind: "prompt"; projectId: string; promptId: string; promptVersionId: string; text: string; modelVersionId: string | null }
  | { kind: "context"; projectId: string; resource: ResourceRef; message: string };
export type LibraryEditRequest =
  | { kind: "prompt"; id: string; text: string; modelVersionId: string | null }
  | { kind: "profile"; id: string; name: string }
  | { kind: "reference-set"; id: string; name: string; description: string; items: { assetId: string; ordinal: number; role: string | null; isPrimary: boolean }[] };
