import { create } from "zustand";
import type {
  DraftValue,
  GenerationValues,
  PendingStudioAssetIntent,
  RecipeViewModel,
  StudioReuseProvenance,
} from "../types/generation";

interface StudioState {
  selectedWorkflow?: RecipeViewModel;
  values: GenerationValues;
  draftDirty: boolean;
  validationErrors: Record<string, string>;
  pendingAssetIntent?: PendingStudioAssetIntent;
  reuseProvenance?: StudioReuseProvenance;
  setSelectedWorkflow: (workflow?: RecipeViewModel) => void;
  loadDraft: (workflow: RecipeViewModel, values: GenerationValues) => void;
  setPendingAssetIntent: (intent: PendingStudioAssetIntent) => void;
  clearPendingAssetIntent: () => void;
  setReuseProvenance: (provenance?: StudioReuseProvenance) => void;
  setValue: (key: string, value: DraftValue) => void;
  removeValue: (key: string) => void;
  setValidationErrors: (errors: Record<string, string>) => void;
  clearValidationErrors: () => void;
  resetDraft: () => void;
}

export const useStudioStore = create<StudioState>((set) => ({
  values: {},
  draftDirty: false,
  validationErrors: {},
  setSelectedWorkflow: (workflow) =>
    set({
      selectedWorkflow: workflow,
      values: workflow ? defaultValues(workflow) : {},
      draftDirty: false,
      validationErrors: {},
      reuseProvenance: undefined,
    }),
  loadDraft: (workflow, values) =>
    set({ selectedWorkflow: workflow, values, draftDirty: false, validationErrors: {} }),
  setPendingAssetIntent: (pendingAssetIntent) => set({ pendingAssetIntent }),
  clearPendingAssetIntent: () => set({ pendingAssetIntent: undefined }),
  setReuseProvenance: (reuseProvenance) => set({ reuseProvenance }),
  setValue: (key, value) =>
    set((state) => ({ values: { ...state.values, [key]: value }, draftDirty: true })),
  removeValue: (key) =>
    set((state) => {
      const values = { ...state.values };
      delete values[key];
      return { values, draftDirty: true };
    }),
  setValidationErrors: (validationErrors) => set({ validationErrors }),
  clearValidationErrors: () => set({ validationErrors: {} }),
  resetDraft: () =>
    set((state) => ({
      values: state.selectedWorkflow ? defaultValues(state.selectedWorkflow) : {},
      draftDirty: false,
      validationErrors: {},
      pendingAssetIntent: undefined,
      reuseProvenance: undefined,
    })),
}));

function defaultValues(workflow: RecipeViewModel): GenerationValues {
  return Object.fromEntries(
    workflow.fields.map((field) => {
      switch (field.type) {
        case "textarea":
          return [field.key, { type: "string", value: field.default }];
        case "integer":
          return field.default === undefined
            ? [field.key, undefined]
            : [field.key, { type: "integer", value: field.default }];
        case "seed":
          return field.defaultMode === "fixed"
            ? [field.key, { type: "seed_fixed", value: field.defaultValue ?? "" }]
            : [field.key, { type: "seed_random" }];
        case "image":
          return [field.key, undefined];
        case "images":
          return [field.key, { type: "image_assets", assetIds: [] }];
        case "video":
        case "audio":
          return [field.key, undefined];
        case "videos":
          return [field.key, { type: "video_assets", assetIds: [] }];
        case "audios":
          return [field.key, { type: "audio_assets", assetIds: [] }];
      }
    }).filter((entry): entry is [string, DraftValue] => Boolean(entry && entry[1] !== undefined)),
  );
}
