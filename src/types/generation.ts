export type RecipeField =
  | {
      key: string;
      type: "textarea";
      label: string;
      required: boolean;
      default: string;
    }
  | {
      key: string;
      type: "integer";
      label: string;
      required: boolean;
      default?: number;
      min?: number;
      max?: number;
      step?: number;
    }
  | {
      key: string;
      type: "seed";
      label: string;
      defaultMode: "random" | "fixed";
      defaultValue?: string | null;
      minValue?: string | null;
      maxValue?: string | null;
    }
  | {
      key: string;
      type: "image";
      label: string;
      required: boolean;
    }
  | {
      key: string;
      type: "images";
      label: string;
      required: boolean;
      minItems: number;
      maxItems: number;
    }
  | {
      key: string;
      type: "video" | "audio";
      label: string;
      required: boolean;
    }
  | {
      key: string;
      type: "videos" | "audios";
      label: string;
      required: boolean;
      minItems: number;
      maxItems: number;
    };

export interface RecipeViewModel {
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  category: string;
  mode: string;
  fields: RecipeField[];
  outputTypes?: Array<"image" | "video">;
}

export type DraftValue =
  | { type: "string"; value: string }
  | { type: "integer"; value: number }
  | { type: "seed_random" }
  | { type: "seed_fixed"; value: string }
  | { type: "image_asset"; assetId: string }
  | { type: "image_assets"; assetIds: string[] }
  | { type: "video_asset"; assetId: string }
  | { type: "audio_asset"; assetId: string }
  | { type: "video_assets"; assetIds: string[] }
  | { type: "audio_assets"; assetIds: string[] };

export type GenerationValues = Record<string, DraftValue>;

export type StudioAssetType = "image" | "video" | "audio";

export interface PendingStudioAssetIntent {
  projectId: string;
  assetId: string;
  assetType: StudioAssetType;
}

export interface StudioReuseProvenance {
  workflowName: string;
  createdAt: string;
  sourceBatchName?: string;
  sourceTaskId?: string;
}
