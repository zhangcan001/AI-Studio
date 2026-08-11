import type { RecipeViewModel } from "../../types/generation";
import { isResolutionAllowedByRecipe } from "./resolution";

export type ResolutionPresetTier = "1k" | "768p" | "2k";

export interface ResolutionPreset {
  id: string;
  label: string;
  width: number;
  height: number;
  ratio: string;
  tier: ResolutionPresetTier;
}

export const KREA2_RESOLUTION_PRESETS: ResolutionPreset[] = [
  { id: "krea2-1k-1x1", label: "1:1", width: 1024, height: 1024, ratio: "1:1", tier: "1k" },
  { id: "krea2-1k-4x3", label: "4:3", width: 1152, height: 864, ratio: "4:3", tier: "1k" },
  { id: "krea2-1k-3x2", label: "3:2", width: 1200, height: 800, ratio: "3:2", tier: "1k" },
  { id: "krea2-1k-16x9", label: "16:9", width: 1280, height: 720, ratio: "16:9", tier: "1k" },
  { id: "krea2-1k-235x1", label: "2.35:1", width: 1504, height: 640, ratio: "2.35:1", tier: "1k" },
  { id: "krea2-1k-4x5", label: "4:5", width: 896, height: 1120, ratio: "4:5", tier: "1k" },
  { id: "krea2-1k-2x3", label: "2:3", width: 800, height: 1200, ratio: "2:3", tier: "1k" },
  { id: "krea2-1k-9x16", label: "9:16", width: 720, height: 1280, ratio: "9:16", tier: "1k" },
  { id: "krea2-2k-1x1", label: "1:1", width: 2048, height: 2048, ratio: "1:1", tier: "2k" },
  { id: "krea2-2k-4x3", label: "4:3", width: 2048, height: 1536, ratio: "4:3", tier: "2k" },
  { id: "krea2-2k-3x2", label: "3:2", width: 1920, height: 1280, ratio: "3:2", tier: "2k" },
  { id: "krea2-2k-16x9", label: "16:9", width: 2048, height: 1152, ratio: "16:9", tier: "2k" },
  { id: "krea2-2k-235x1", label: "2.35:1", width: 1920, height: 816, ratio: "2.35:1", tier: "2k" },
  { id: "krea2-2k-4x5", label: "4:5", width: 1600, height: 2000, ratio: "4:5", tier: "2k" },
  { id: "krea2-2k-2x3", label: "2:3", width: 1280, height: 1920, ratio: "2:3", tier: "2k" },
  { id: "krea2-2k-9x16", label: "9:16", width: 1152, height: 2048, ratio: "9:16", tier: "2k" },
];

export const MINIMAX_H3_RESOLUTION_PRESETS: ResolutionPreset[] = [
  { id: "h3-0-2mp-16x9", label: "0.2 MP · 16:9", width: 608, height: 352, ratio: "16:9", tier: "768p" },
  { id: "h3-0-3mp-16x9", label: "0.3 MP · 16:9", width: 736, height: 416, ratio: "16:9", tier: "768p" },
  { id: "h3-0-4mp-16x9", label: "0.4 MP · 16:9", width: 864, height: 480, ratio: "16:9", tier: "768p" },
  { id: "h3-0-5mp-16x9", label: "0.5 MP · 16:9", width: 960, height: 544, ratio: "16:9", tier: "768p" },
  { id: "h3-0-6mp-16x9", label: "0.6 MP · 16:9", width: 1056, height: 608, ratio: "16:9", tier: "768p" },
  { id: "h3-0-7mp-16x9", label: "0.7 MP · 16:9", width: 1152, height: 640, ratio: "16:9", tier: "768p" },
  { id: "h3-0-8mp-16x9", label: "0.8 MP · 16:9", width: 1216, height: 672, ratio: "16:9", tier: "768p" },
  { id: "h3-0-9mp-16x9", label: "0.9 MP · 16:9", width: 1280, height: 736, ratio: "16:9", tier: "768p" },
  { id: "h3-0-98mp-16x9", label: "0.98 MP · 16:9", width: 1344, height: 768, ratio: "16:9", tier: "768p" },
  { id: "h3-1-0mp-16x9", label: "1.0 MP · 16:9", width: 1376, height: 768, ratio: "16:9", tier: "768p" },
  { id: "h3-1-2mp-16x9", label: "1.2 MP · 16:9", width: 1504, height: 832, ratio: "16:9", tier: "768p" },
  { id: "h3-1-5mp-16x9", label: "1.5 MP · 16:9", width: 1664, height: 928, ratio: "16:9", tier: "768p" },
  { id: "h3-1-8mp-16x9", label: "1.8 MP · 16:9", width: 1824, height: 1024, ratio: "16:9", tier: "768p" },
  { id: "h3-2-0mp-16x9", label: "2.0 MP · 16:9", width: 1920, height: 1088, ratio: "16:9", tier: "768p" },
];

export function isMinimaxH3OutputResolution(width: number, height: number): boolean {
  return MINIMAX_H3_RESOLUTION_PRESETS.some((preset) => preset.width === width && preset.height === height);
}

export function resolutionPresetsForRecipe(
  recipe: RecipeViewModel,
  presets: ResolutionPreset[],
): ResolutionPreset[] {
  return presets.filter((preset) => isResolutionAllowedByRecipe(recipe, preset.width, preset.height));
}
