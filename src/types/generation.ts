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
    }
  | {
      key: string;
      type: "seed";
      label: string;
      defaultMode: "random" | "fixed";
      defaultValue?: string | null;
      minValue?: string | null;
      maxValue?: string | null;
    };

export interface RecipeViewModel {
  workflowId: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  category: string;
  mode: string;
  fields: RecipeField[];
}

export type DraftValue =
  | { type: "string"; value: string }
  | { type: "integer"; value: number }
  | { type: "seed_random" }
  | { type: "seed_fixed"; value: string };

export type GenerationValues = Record<string, DraftValue>;
