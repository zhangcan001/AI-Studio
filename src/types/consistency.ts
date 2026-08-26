/**
 * Frontend-facing contracts for the consistency asset workspace.
 *
 * The backend intentionally exposes a stable, flat view instead of leaking
 * Rust's externally-tagged profile enum into the UI.  Keep optional fields
 * nullable because older projects and partially populated records are valid.
 */

export const consistencyProfileTypes = ["CHARACTER", "SCENE", "PROP", "STYLE"] as const;
export type ProfileType = (typeof consistencyProfileTypes)[number];

export const referenceSetPurposes = [
  "CHARACTER",
  "COSTUME",
  "SCENE",
  "PROP",
  "STYLE",
  "SHOT",
] as const;
export type ReferenceSetPurpose = (typeof referenceSetPurposes)[number];

export const MAX_REFERENCE_SET_ITEMS = 20;

export interface ConsistencyProfileView {
  id: string;
  projectId: string;
  profileType: ProfileType;
  name: string;
  description: string;
  canonicalPrompt?: string | null;
  negativePrompt?: string | null;
  environmentPrompt?: string | null;
  lightingPrompt?: string | null;
  materialPrompt?: string | null;
  scalePrompt?: string | null;
  stylePrompt?: string | null;
  colorPrompt?: string | null;
  linePrompt?: string | null;
  outputNotes?: string | null;
  defaultReferenceSetId?: string | null;
  defaultStyleProfileId?: string | null;
  activeRevisionId?: string | null;
  metadataJson?: string | null;
  createdAt: string;
  updatedAt: string;
}

/** Editable flat form model used by the profile editor. */
export interface ConsistencyProfileDraft {
  profileType: ProfileType;
  name: string;
  description: string;
  canonicalPrompt: string;
  negativePrompt: string;
  environmentPrompt: string;
  lightingPrompt: string;
  materialPrompt: string;
  scalePrompt: string;
  stylePrompt: string;
  colorPrompt: string;
  linePrompt: string;
  outputNotes: string;
  defaultReferenceSetId: string;
  defaultStyleProfileId: string;
  metadataJson: string;
}

export interface CharacterProfileRequest {
  projectId: string;
  name: string;
  description: string;
  canonicalPrompt: string;
  negativePrompt: string;
  defaultStyleProfileId?: string | null;
  defaultReferenceSetId?: string | null;
  metadataJson?: string;
}

export interface CharacterProfileUpdateRequest extends CharacterProfileRequest {
  profileId: string;
}

export interface SceneProfileRequest {
  projectId: string;
  name: string;
  description: string;
  environmentPrompt: string;
  lightingPrompt?: string | null;
  negativePrompt?: string | null;
  defaultStyleProfileId?: string | null;
  defaultReferenceSetId?: string | null;
}

export interface SceneProfileUpdateRequest extends SceneProfileRequest {
  profileId: string;
}

export interface PropProfileRequest {
  projectId: string;
  name: string;
  description: string;
  canonicalPrompt: string;
  materialPrompt?: string | null;
  scalePrompt?: string | null;
  defaultReferenceSetId?: string | null;
}

export interface PropProfileUpdateRequest extends PropProfileRequest {
  profileId: string;
}

export interface StyleProfileRequest {
  projectId: string;
  name: string;
  stylePrompt: string;
  colorPrompt?: string | null;
  linePrompt?: string | null;
  negativePrompt?: string | null;
  outputNotes?: string | null;
}

export interface StyleProfileUpdateRequest extends StyleProfileRequest {
  profileId: string;
}

export interface CostumeVariantView {
  id: string;
  characterProfileId: string;
  name: string;
  promptFragment: string;
  referenceSetId?: string | null;
  isDefault: boolean;
  ordinal: number;
  activeRevisionId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CostumeVariantRequest {
  projectId: string;
  characterProfileId: string;
  name: string;
  promptFragment: string;
  referenceSetId?: string | null;
  isDefault: boolean;
  ordinal: number;
}

export interface CostumeVariantUpdateRequest {
  projectId: string;
  costumeVariantId: string;
  name: string;
  promptFragment: string;
  referenceSetId?: string | null;
  isDefault: boolean;
  ordinal: number;
}

export interface ReferenceSetView {
  id: string;
  projectId: string;
  name: string;
  purpose: ReferenceSetPurpose;
  description: string;
  ownerProfileType?: ProfileType | null;
  ownerProfileId?: string | null;
  ownerProfileName?: string | null;
  activeRevisionId?: string | null;
  itemCount?: number;
  imageCount?: number;
  createdAt: string;
  updatedAt: string;
}

export type ReferenceSetSummary = ReferenceSetView;

export interface ReferenceSetItemView {
  referenceSetId?: string;
  assetId: string;
  ordinal: number;
  role?: string | null;
  isPrimary: boolean;
  assetName?: string | null;
  thumbnailAvailable?: boolean;
  width?: number | null;
  height?: number | null;
}

export interface ReferenceSetDetailView {
  referenceSet: ReferenceSetView;
  items: ReferenceSetItemView[];
}

export interface ReferenceSetItemInput {
  assetId: string;
  ordinal: number;
  role?: string | null;
  isPrimary: boolean;
}

export interface ReferenceSetDraft {
  name: string;
  purpose: ReferenceSetPurpose;
  description: string;
  ownerProfileType?: ProfileType | null;
  ownerProfileId?: string | null;
  items: ReferenceSetItemInput[];
}

export interface ReferenceSetRequest extends ReferenceSetDraft {
  projectId: string;
}

export interface ReferenceSetUpdateRequest extends ReferenceSetRequest {
  referenceSetId: string;
}

export interface UsageRelation {
  entityType?: string | null;
  entityId?: string | null;
  displayName?: string | null;
  relationType?: string | null;
  scopeType?: string | null;
  scopeId?: string | null;
  shotId?: string | null;
  profileType?: ProfileType | string | null;
  profileId?: string | null;
  referenceSetId?: string | null;
  blocking?: boolean;
  detail?: string | null;
}

export interface AssetUsageSummary {
  assetId: string;
  total: number;
  blockingCount: number;
  referenceSets: UsageRelation[];
  profiles: UsageRelation[];
  shots: UsageRelation[];
  legacyReferences: UsageRelation[];
  productionHistory: UsageRelation[];
  selectedKeyframes?: UsageRelation[];
  items: UsageRelation[];
}

export interface ProfileUsageSummary {
  profileId: string;
  profileType: ProfileType;
  total: number;
  blockingCount: number;
  shotBindings: UsageRelation[];
  scopeBindings: UsageRelation[];
  referenceSets: UsageRelation[];
  defaultStyleProfiles: UsageRelation[];
  costumeVariants: UsageRelation[];
  relatedProfiles: UsageRelation[];
  items: UsageRelation[];
}

export interface ReferenceSetUsageSummary {
  referenceSetId: string;
  total: number;
  blockingCount: number;
  profileDefaults: UsageRelation[];
  costumes: UsageRelation[];
  shotBindings: UsageRelation[];
  scopeBindings: UsageRelation[];
  owner?: UsageRelation | null;
  itemCount: number;
  items: UsageRelation[];
}

export function isProfileType(value: string): value is ProfileType {
  return (consistencyProfileTypes as readonly string[]).includes(value);
}

export function isReferenceSetPurpose(value: string): value is ReferenceSetPurpose {
  return (referenceSetPurposes as readonly string[]).includes(value);
}
