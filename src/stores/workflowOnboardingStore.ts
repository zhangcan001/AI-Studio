import { create } from "zustand";
import type { WorkflowOnboardingDraftView } from "../types/workflowOnboarding";

export type WorkflowOnboardingStep =
  | "inspect"
  | "compatibility"
  | "inputs"
  | "outputs"
  | "metadata"
  | "validate"
  | "publish";

interface WorkflowOnboardingState {
  draft?: WorkflowOnboardingDraftView;
  step: WorkflowOnboardingStep;
  loading: boolean;
  error?: string;
  notice?: string;
  setDraft: (draft?: WorkflowOnboardingDraftView) => void;
  updateDraft: (draft: WorkflowOnboardingDraftView) => void;
  setStep: (step: WorkflowOnboardingStep) => void;
  setLoading: (loading: boolean) => void;
  setError: (error?: string) => void;
  setNotice: (notice?: string) => void;
  reset: () => void;
}

export const useWorkflowOnboardingStore = create<WorkflowOnboardingState>((set) => ({
  step: "inspect",
  loading: false,
  setDraft: (draft) => set({ draft, step: "inspect", error: undefined, notice: undefined }),
  updateDraft: (draft) => set({ draft, error: undefined }),
  setStep: (step) => set({ step, error: undefined }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  setNotice: (notice) => set({ notice }),
  reset: () => set({ draft: undefined, step: "inspect", loading: false, error: undefined, notice: undefined }),
}));
