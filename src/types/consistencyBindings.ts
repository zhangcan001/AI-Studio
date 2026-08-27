export const consistencyScopeTypes = ["PROJECT", "SERIES", "EPISODE", "SCENE", "SHOT"] as const;
export type ConsistencyScopeType = (typeof consistencyScopeTypes)[number];

export const consistencyBindingRoles = ["CHARACTER", "SCENE", "PROP", "STYLE"] as const;
export type ConsistencyProfileBindingRole = (typeof consistencyBindingRoles)[number];

export const consistencyReferenceSetRoles = ["CHARACTER", "SCENE", "PROP", "STYLE", "SHOT_REFERENCE"] as const;
export type ConsistencyReferenceSetBindingRole = (typeof consistencyReferenceSetRoles)[number];

export const consistencyInheritanceModes = ["EXPLICIT", "REPLACE", "REMOVE", "INHERITED"] as const;
export type ConsistencyInheritanceMode = (typeof consistencyInheritanceModes)[number];

export type ConsistencyProfileType = "CHARACTER" | "SCENE" | "PROP" | "STYLE";
export type ConsistencyDiagnosticSeverity = "ERROR" | "WARNING" | "INFO";
export type ConsistencyContextSourceScope = ConsistencyScopeType | "LEGACY";

export interface ConsistencyScopeRef {
  scopeType: ConsistencyScopeType;
  scopeId: string;
  scopeName: string;
}

export interface ConsistencyProfileBindingInput {
  id?: string;
  role: ConsistencyProfileBindingRole;
  profileType: ConsistencyProfileType;
  profileId: string;
  costumeVariantId?: string | null;
  ordinal: number;
  inheritanceMode: ConsistencyInheritanceMode;
}

export interface ConsistencyReferenceSetBindingInput {
  id?: string;
  role: ConsistencyReferenceSetBindingRole;
  referenceSetId: string;
  ordinal: number;
  required: boolean;
  inheritanceMode: ConsistencyInheritanceMode;
}

export interface ConsistencyBindingReplaceInput {
  projectId: string;
  scopeType: ConsistencyScopeType;
  scopeId: string;
  profileBindings: ConsistencyProfileBindingInput[];
  referenceSetBindings: ConsistencyReferenceSetBindingInput[];
}

export interface ConsistencyAncestorBindingSummary extends ConsistencyScopeRef {
  profileBindings: ConsistencyProfileBindingInput[];
  referenceSetBindings: ConsistencyReferenceSetBindingInput[];
}

export interface ConsistencyBindingPack {
  scope: ConsistencyScopeRef;
  ancestors: ConsistencyAncestorBindingSummary[];
  directProfileBindings: ConsistencyProfileBindingInput[];
  directReferenceSetBindings: ConsistencyReferenceSetBindingInput[];
}

export interface ConsistencyProfileOption {
  id: string;
  projectId: string;
  profileType: ConsistencyProfileType;
  name: string;
  description?: string | null;
}

export interface ConsistencyCostumeOption {
  id: string;
  characterProfileId: string;
  name: string;
  promptFragment?: string | null;
  referenceSetId?: string | null;
  isDefault?: boolean;
}

export interface ConsistencyReferenceSetOption {
  id: string;
  projectId: string;
  name: string;
  purpose: "CHARACTER" | "COSTUME" | "SCENE" | "PROP" | "STYLE" | "SHOT";
  itemCount?: number;
  imageCount?: number;
}

export interface ConsistencySourceTrace {
  scope: ConsistencyContextSourceScope;
  scopeId?: string | null;
  scopeName?: string | null;
}

export interface ConsistencyResolvedProfile {
  role: ConsistencyProfileBindingRole;
  profileType: ConsistencyProfileType;
  profileId: string;
  name: string;
  ordinal: number;
  source: ConsistencySourceTrace;
  costumeVariantId?: string | null;
  costumeName?: string | null;
}

export interface ConsistencyResolvedReferenceSet {
  role: ConsistencyReferenceSetBindingRole;
  referenceSetId: string;
  name: string;
  ordinal: number;
  required: boolean;
  source: ConsistencySourceTrace;
  assetCount?: number;
  previewAssets?: ConsistencyReferencePreviewAsset[];
}

export interface ConsistencyReferencePreviewAsset {
  assetId: string;
  name?: string | null;
  thumbnailUrl?: string | null;
  ordinal?: number;
}

export interface ConsistencyDiagnostic {
  severity: ConsistencyDiagnosticSeverity;
  code: string;
  message: string;
}

export interface ConsistencyContextPreview {
  contextHash?: string | null;
  partial: boolean;
  diagnostics: ConsistencyDiagnostic[];
  sourceTrace?: ConsistencySourceTrace[];
  profiles?: ConsistencyResolvedProfile[];
  referenceSets?: ConsistencyResolvedReferenceSet[];
  promptText?: string | null;
  negativePrompt?: string | null;
  readinessStatus?: string | null;
  legacy?: {
    usesLegacyShotReferences: boolean;
    prompt?: string | null;
  };
}

export interface ConsistencyScopeOption extends ConsistencyScopeRef {
  parentName?: string | null;
}

export interface ConsistencyLoadError {
  message: string;
  code?: string;
}

export function roleLabel(role: ConsistencyProfileBindingRole | ConsistencyReferenceSetBindingRole): string {
  return {
    CHARACTER: "角色",
    COSTUME: "服装",
    SCENE: "场景",
    PROP: "道具",
    STYLE: "风格",
    SHOT_REFERENCE: "镜头参考",
  }[role];
}

export function scopeLabel(scopeType: ConsistencyScopeType): string {
  return {
    PROJECT: "项目",
    SERIES: "系列",
    EPISODE: "集",
    SCENE: "场景",
    SHOT: "镜头",
  }[scopeType];
}

export function sourceLabel(source: ConsistencySourceTrace): string {
  return source.scope === "LEGACY" ? "旧版" : scopeLabel(source.scope);
}

export function profileTypeForRole(role: ConsistencyProfileBindingRole): ConsistencyProfileType {
  return role;
}

export function normalizeBindingOrdinals<T extends { role: string; ordinal: number }>(bindings: readonly T[]): T[] {
  const nextOrdinal = new Map<string, number>();
  return bindings.map((binding) => {
    const ordinal = nextOrdinal.get(binding.role) ?? 0;
    nextOrdinal.set(binding.role, ordinal + 1);
    return { ...binding, ordinal };
  });
}
