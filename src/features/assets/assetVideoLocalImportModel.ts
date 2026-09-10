import type { H3ProjectGenerationMode, H3ProjectSegment } from "../../types/h3LocalImport";

export interface ProjectSegmentForm {
  mode: H3ProjectGenerationMode;
  prompt: string;
  durationSeconds: number;
  width: number;
  height: number;
  referenceImageIds: string[];
  referenceAudioIds: string[];
  referenceVideoIds: string[];
  firstFrameId?: string;
  lastFrameId?: string;
}

export function projectSegmentForm(segment: H3ProjectSegment): ProjectSegmentForm {
  return {
    mode: segment.generationMode,
    prompt: segment.prompt ?? "",
    durationSeconds: segment.durationSeconds,
    width: segment.width,
    height: segment.height,
    referenceImageIds: segment.referenceImages.map((media) => media.id),
    referenceAudioIds: segment.referenceAudios.map((media) => media.id),
    referenceVideoIds: segment.referenceVideos.map((media) => media.id),
    firstFrameId: segment.firstFrame?.id,
    lastFrameId: segment.lastFrame?.id,
  };
}
