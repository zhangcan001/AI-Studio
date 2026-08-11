export type H3LocalImportMode = "PAIRING" | "MANIFEST";

export type H3LocalPairStatus =
  | "READY"
  | "MISSING_PROMPT"
  | "MISSING_IMAGE"
  | "AMBIGUOUS_PROMPT"
  | "AMBIGUOUS_IMAGE"
  | "INVALID_PROMPT_ENCODING"
  | "EMPTY_PROMPT"
  | "PROMPT_TOO_LARGE"
  | "INVALID_IMAGE"
  | "IMAGE_TOO_LARGE"
  | "INVALID_PATH"
  | "DUPLICATE_IMAGE_ENTRY"
  | "UNKNOWN_IMAGE";

export interface H3LocalImportPair {
  ordinal: number;
  imageDisplayName: string;
  promptDisplayName: string;
  promptPreview?: string;
  promptBytes?: number;
  status: H3LocalPairStatus;
}

export interface H3LocalImportInspection {
  sessionId: string;
  displayRootName: string;
  mode: H3LocalImportMode;
  detectedManifest: boolean;
  imageCount: number;
  promptCount: number;
  readyCount: number;
  errorCount: number;
  pairs: H3LocalImportPair[];
  errors: string[];
  warnings: string[];
}

export interface H3LocalImportResult {
  batchId: string;
  batchName: string;
  itemCount: number;
  importedAssetCount: number;
  autoStarted: boolean;
  warnings: string[];
}
