export type ProductionPackageItemStatus = "READY" | "BLOCKED" | "WARNING" | string;

/** The inspection DTO is intentionally a display contract, not a persistence model. */
export type ProductionPackageInspectionStatus = ProductionPackageItemStatus;

export type ProductionPackageMediaKind = "image" | "video" | "audio" | string;

export interface ProductionPackageMediaMetadata {
  relativePath?: string | null;
  path?: string | null;
  fileName?: string | null;
  displayName?: string | null;
  kind?: ProductionPackageMediaKind | null;
  mimeType?: string | null;
  sizeBytes?: number | null;
  width?: number | null;
  height?: number | null;
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
  detail?: string | null;
}

export type ProductionPackageIssue = string | ProductionPackageDiagnostic;

export interface ProductionPackageInspectionItem {
  id: string;
  name: string;
  mode?: string | null;
  videoPromptPreview?: string | null;
  firstFrame?: ProductionPackageMedia | null;
  lastFrame?: ProductionPackageMedia | null;
  references?: ProductionPackageMedia[];
  referenceImages?: ProductionPackageMedia[];
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
  packageType?: string;
}
