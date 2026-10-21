export type FolderImportMode = "firstLast" | "reference" | "image";
export interface FolderImportIssue { code: string; message: string; rowNumber?: number; shotId?: string }
export interface FolderImportRow {
  rowKey: string; rowNumber: number; manifestId: string | null; mode: FolderImportMode; name: string;
  promptPreview: string; durationSeconds: number; width: number; height: number; generatorLabel: string;
  firstFrame: string | null; lastFrame: string | null; referenceCount: number; entityNames: string[];
  errors: FolderImportIssue[]; warnings: FolderImportIssue[];
}
export interface FolderImportPreview {
  sessionId: string; previewVersion: number; nameSuffix: string | null; displayRootName: string;
  manifestSha256: string; formatSource: "manifest" | "h3-batch" | "h3-omni-batch";
  shotCount: number; validCount: number; errorCount: number; warningCount: number; maxQueueItems: number; createQueueDefault: boolean;
  characters: Array<{ name: string; kind: string; action: "reuse" | "create"; imageCount: number; existingEntityId: string | null }>;
  rows: FolderImportRow[]; errors: FolderImportIssue[]; warnings: string[];
}

export interface FolderImportCommitRequest {
 sessionId:string;previewVersion:number;manifestSha256:string;includeRowKeys:string[];
 createQueue:boolean;autoStart:boolean;nameSuffix:string|null;
}
export interface FolderImportResult {
 outcome:"completed"|"compensated"|"partial";
 failure:{stage:string;code:string;message:string}|null;
 compensationErrors:{target:string;id:string;message:string}[];
 createdShotIds:string[];reusedEntityIds:string[];createdEntityIds:string[];importedAssetIds:string[];
 batchId:string|null;itemCount:number;autoStarted:boolean;
 residual:{unverifiedFileCleanup:boolean;shotIds:string[];entityIds:string[];assetIds:string[];batchId:string|null};warnings:string[];
}

export interface FolderImportLegacyDefaults {selectionRef:string;durationSeconds:number;width:number;height:number}
export interface FolderImportPreviewOptions {nameSuffix:string|null;legacy?:FolderImportLegacyDefaults|null}
