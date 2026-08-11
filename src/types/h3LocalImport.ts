export type H3LocalImportMode = "PAIRING" | "MANIFEST" | "TEXT" | "FIRST_LAST" | "OMNI_MANIFEST" | "PROJECT_FOLDER";

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
  lastImageDisplayName?: string;
  videoDisplayNames?: string[];
  audioDisplayNames?: string[];
}

export type H3ProjectGenerationMode =
  | "FL2VA_TEXT_TO_VIDEO"
  | "FL2VA_IMAGE_TO_VIDEO"
  | "FL2VA_FIRST_LAST"
  | "REF2VA_IMAGE"
  | "REF2VA_AUDIO"
  | "REF2VA_IMAGE_AUDIO"
  | "REF2VA_VIDEO_IMAGE";

export interface H3ProjectMedia {
  id: string;
  displayName: string;
  kind: "image" | "audio" | "video";
  sizeBytes: number;
  width?: number;
  height?: number;
  durationMs?: number;
}

export interface H3ProjectSegment {
  ordinal: number;
  segmentId: string;
  folderName: string;
  generationMode: H3ProjectGenerationMode;
  inferredMode: H3ProjectGenerationMode;
  modeSource: "AUTO_INFERENCE" | "FRONT_MATTER" | "USER_OVERRIDE" | string;
  prompt?: string;
  promptDisplayName?: string;
  promptBytes?: number;
  width: number;
  height: number;
  resolutionSource: string;
  durationSeconds: number;
  durationSource: string;
  firstFrame?: H3ProjectMedia;
  lastFrame?: H3ProjectMedia;
  referenceImages: H3ProjectMedia[];
  referenceAudios: H3ProjectMedia[];
  referenceVideos: H3ProjectMedia[];
  media: H3ProjectMedia[];
  status: "READY" | "BLOCKED" | string;
  errors: string[];
  warnings: string[];
}

export interface H3ProjectFolderInspection {
  displayRootName: string;
  segmentCount: number;
  readyCount: number;
  errorCount: number;
  segments: H3ProjectSegment[];
  errors: string[];
  warnings: string[];
}

export interface H3ProjectSegmentDraft {
  sessionId: string;
  segmentId: string;
  mode?: H3ProjectGenerationMode;
  prompt?: string;
  durationSeconds?: number;
  width?: number;
  height?: number;
  referenceImageIds?: string[];
  referenceAudioIds?: string[];
  referenceVideoIds?: string[];
  firstFrameId?: string;
  lastFrameId?: string;
  resetAutoDetection?: boolean;
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
  projectFolder?: H3ProjectFolderInspection;
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
