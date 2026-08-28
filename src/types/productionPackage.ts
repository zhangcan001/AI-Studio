export type ProductionPackageItemStatus = "READY" | "BLOCKED" | "WARNING" | string;

/** The inspection DTO is intentionally a display contract, not a persistence model. */
export type ProductionPackageInspectionStatus = ProductionPackageItemStatus;

export type ProductionPackageMediaKind = "image" | "video" | "audio" | string;

export type ProductionPackageDiagnosticSeverity = "INFO" | "WARNING" | "ERROR" | "BLOCKER" | string;

export interface ProductionPackageDefaults {
  durationSeconds?: number | null;
  width?: number | null;
  height?: number | null;
  mode?: string | null;
}

export interface ProductionPackageMediaMetadata {
  relativePath?: string | null;
  path?: string | null;
  fileName?: string | null;
  displayName?: string | null;
  kind?: ProductionPackageMediaKind | null;
  regularFile?: boolean;
  readable?: boolean;
  format?: string | null;
  mimeType?: string | null;
  sizeBytes?: number | null;
  width?: number | null;
  height?: number | null;
  durationMs?: number | null;
  durationSeconds?: number | null;
  sha256?: string | null;
  exists?: boolean;
}

export type ProductionPackageMedia = string | ProductionPackageMediaMetadata;

export interface ProductionPackageResolution {
  width?: number | null;
  height?: number | null;
}

export type ProductionPackageDuration =
  | number
  | string
  | {
      seconds?: number | null;
      durationSeconds?: number | null;
    };

export interface ProductionPackageDiagnostic {
  code?: string | null;
  message?: string | null;
  severity?: ProductionPackageDiagnosticSeverity | null;
  field?: string | null;
  detail?: string | null;
}

export type ProductionPackageIssue = string | ProductionPackageDiagnostic;

export interface ProductionPackageInspectionItem {
  id: string;
  name: string;
  text?: string | null;
  imagePrompt?: string | null;
  videoPrompt?: string | null;
  episode?: string | null;
  scene?: string | null;
  mode?: string | null;
  videoPromptPreview?: string | null;
  firstFrame?: ProductionPackageMedia | null;
  lastFrame?: ProductionPackageMedia | null;
  references?: ProductionPackageMedia[];
  referenceImages?: ProductionPackageMedia[];
  referenceAudios?: ProductionPackageMedia[];
  referenceVideos?: ProductionPackageMedia[];
  duration?: ProductionPackageDuration | null;
  durationSeconds?: number | null;
  resolution?: ProductionPackageResolution | string | null;
  width?: number | null;
  height?: number | null;
  status: ProductionPackageItemStatus;
  warnings?: ProductionPackageIssue[];
  errors?: ProductionPackageIssue[];
}

export interface ProductionPackageInspection {
  packageName: string;
  itemCount: number;
  readyCount: number;
  warningCount: number;
  blockedCount: number;
  items: ProductionPackageInspectionItem[];
  packageId?: string | null;
  schemaVersion?: number;
  packageType?: string | null;
  defaults?: ProductionPackageDefaults | null;
  manifestSha256?: string | null;
  status?: ProductionPackageInspectionStatus;
  warnings?: ProductionPackageIssue[];
  errors?: ProductionPackageIssue[];
}

export interface ProductionPackageInspectionResult extends ProductionPackageInspection {
  inspectionId: string;
}

export interface ProductionPackageItemMapping {
  packageItemId: string;
  batchId: string;
  batchItemId: string;
  importedAssetIds: string[];
}

export interface ProductionPackageBatchMapping {
  batchId: string;
  batchName: string;
  itemCount: number;
  itemMappings: ProductionPackageItemMapping[];
}

export type ProductionPackageCreateStatus = "COMPLETE" | "PARTIAL" | string;

export interface ProductionPackageCreateBatchesResult {
  packageName: string;
  status: ProductionPackageCreateStatus;
  requestedCount: number;
  createdCount: number;
  remainingCount: number;
  remainingItemIds: string[];
  batchCount: number;
  itemCount: number;
  autoStarted: boolean;
  batches: ProductionPackageBatchMapping[];
  itemMappings: ProductionPackageItemMapping[];
  warnings: ProductionPackageDiagnostic[];
}

export interface ProductionPackageInspectRequest {
  projectId: string;
  packageRoot: string;
}

/** Commit submits only the short-lived inspection and selected external labels. */
export interface ProductionPackageCreateBatchesRequest {
  inspectionId: string;
  selectedItemIds: string[];
}
