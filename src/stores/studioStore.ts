import { create } from "zustand";
import type { DraftValue, GenerationValues, RecipeViewModel } from "../types/generation";

interface StudioState {
  selectedWorkflow?: RecipeViewModel;
  values: GenerationValues;
  validationErrors: Record<string, string>;
  setSelectedWorkflow: (workflow?: RecipeViewModel) => void;
  loadDraft: (workflow: RecipeViewModel, values: GenerationValues) => void;
  setValue: (key: string, value: DraftValue) => void;
  removeValue: (key: string) => void;
  setValidationErrors: (errors: Record<string, string>) => void;
  clearValidationErrors: () => void;
  resetDraft: () => void;
}

export const useStudioStore = create<StudioState>((set) => ({
  values: {},
  validationErrors: {},
  setSelectedWorkflow: (workflow) =>
    set({
      selectedWorkflow: workflow,
      values: workflow ? defaultValues(workflow) : {},
      validationErrors: {},
    }),
  loadDraft: (workflow, values) =>
    set({ selectedWorkflow: workflow, values, validationErrors: {} }),
  setValue: (key, value) =>
    set((state) => ({ values: { ...state.values, [key]: value } })),
  removeValue: (key) =>
    set((state) => {
      const values = { ...state.values };
      delete values[key];
      return { values };
    }),
  setValidationErrors: (validationErrors) => set({ validationErrors }),
  clearValidationErrors: () => set({ validationErrors: {} }),
  resetDraft: () =>
    set((state) => ({
      values: state.selectedWorkflow ? defaultValues(state.selectedWorkflow) : {},
      validationErrors: {},
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
      }
    }).filter((entry): entry is [string, DraftValue] => entry[1] !== undefined),
  );
}
