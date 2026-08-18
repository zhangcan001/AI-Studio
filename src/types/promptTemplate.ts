import type { ShotStage } from "./shot";

export interface PromptTemplateAnalysis {
  isTemplate: boolean;
  variables: string[];
  builtinVariables: string[];
  customVariables: string[];
  requiresStructure: boolean;
}

export interface PromptTemplatePreviewRequest {
  projectId: string;
  promptEntryId: string;
  promptVersionId: string;
  shotId: string;
  contextAnchorIds: string[];
  customValues: Record<string, string>;
}

export interface PromptTemplatePreview {
  shotId: string;
  shotName: string;
  templateText: string;
  renderedText: string;
  variables: string[];
  context: unknown;
  warnings: string[];
}

export interface PromptTemplateBulkPreviewRequest {
  projectId: string;
  promptEntryId: string;
  promptVersionId: string;
  shotIds: string[];
  contextAnchorIds: string[];
  customValues: Record<string, string>;
  previewLimit?: number;
}

export interface PromptTemplateBulkPreviewEntry {
  shotId: string;
  shotName: string;
  renderedText: string;
  variables: string[];
}

export interface PromptTemplateIssue {
  shotId?: string;
  shotName?: string;
  code: string;
  message: string;
}

export interface PromptTemplateBulkPreview {
  total: number;
  valid: number;
  invalid: number;
  previewEntries: PromptTemplateBulkPreviewEntry[];
  issues: PromptTemplateIssue[];
}

export interface PromptTemplateApplyRequest {
  projectId: string;
  promptEntryId: string;
  promptVersionId: string;
  stage: ShotStage;
  shotIds: string[];
  contextAnchorIds: string[];
  customValues: Record<string, string>;
}

export interface PromptTemplateApplyResult {
  stage: ShotStage;
  updatedCount: number;
  shotIds: string[];
  updatedAt: string;
}
