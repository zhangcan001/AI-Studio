import type { PageCursor } from "./asset";

export type PromptKind = "prompt" | "snippet";

export interface PromptVersionView {
  id: string;
  promptId: string;
  version: number;
  text: string;
  createdAt: string;
}

export interface PromptEntryView {
  id: string;
  projectId: string;
  kind: PromptKind;
  name: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  versionCount: number;
  versions: PromptVersionView[];
}

export interface PromptLibraryCreateRequest {
  projectId: string;
  kind: PromptKind;
  name: string;
  tags: string[];
  text: string;
}

export interface PromptLibraryMetadataRequest {
  projectId: string;
  promptId: string;
  name: string;
  tags: string[];
}

export interface PromptLibraryPage {
  items: PromptEntryView[];
  nextCursor?: PageCursor;
}
