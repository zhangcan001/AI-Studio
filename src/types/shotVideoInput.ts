export type VideoInputKey = "first_frame" | "last_frame" | "reference_images" | "reference_videos" | "reference_audios";
export interface VideoInputAsset { inputKey: VideoInputKey; ordinal: number; assetId: string }
export interface VideoInputToken { instanceId: string; revision: number }
export interface ShotVideoInputScope { projectId: string; shotId: string; workflowVersionId: string; recipeId: string }
export interface ShotVideoInputSet { scope: ShotVideoInputScope; token: VideoInputToken; inputs: VideoInputAsset[]; updatedAt: string }
export interface InputAsset { id: string; name: string; mediaKind: "image" | "video" | "audio" }
export interface VideoInputSelection { projectId: string; shotId: string; selectionRef: string }
export interface VideoInputView { selectionRef: string; token: VideoInputToken | null; inputs: VideoInputAsset[]; assets: InputAsset[] }
export interface VideoInputSave { selection: VideoInputSelection; expected: VideoInputToken | null; inputs: VideoInputAsset[] }
export interface SourceInputImport { imported: InputAsset[]; failed: { displayName: string; error: string }[]; cancelled: boolean }
