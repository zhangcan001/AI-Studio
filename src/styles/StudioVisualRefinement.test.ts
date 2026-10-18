// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const tokens = readFileSync('src/styles/studioTokens.css', 'utf8');
const quality = readFileSync('src/styles/studioQuality.css', 'utf8');
function color(name: string) {
  const hex = tokens.match(new RegExp(`--studio-${name}:\\s*(#[0-9a-f]{6})`, 'i'))?.[1];
  if (!hex) throw Error(`Missing literal base color: ${name}`);
  return [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16));
}
function luminance(rgb: number[]) {
  return rgb.map(value => { const v = value / 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; })
    .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
}
function contrast(a: number[], b: number[]) {
  const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

describe('desktop visual refinement', () => {
  it('uses quiet neutral surfaces instead of a saturated blue canvas', () => {
    for (const name of ['bg', 'surface-1', 'surface-2']) {
      const rgb = color(name);
      expect(Math.max(...rgb) - Math.min(...rgb)).toBeLessThanOrEqual(16);
    }
  });
  it('keeps normal text and action labels at AA contrast on each surface', () => {
    for (const foreground of ['text', 'text-secondary', 'text-muted', 'accent']) {
      for (const background of ['bg', 'surface-1', 'surface-2', 'surface-3']) {
        expect(contrast(color(foreground), color(background)), `${foreground}/${background}`).toBeGreaterThanOrEqual(4.5);
      }
    }
    for (const name of ['accent-strong', 'accent-primary-hover']) {
      expect(contrast([255, 255, 255], color(name))).toBeGreaterThanOrEqual(4.5);
    }
  });
  it('shares control, focus and semantic colors instead of a purple hover override', () => {
    expect(quality).not.toContain('background: #8b5cf6');
    expect(quality).toContain('background: var(--studio-accent-primary-hover)');
    expect(tokens).toContain('--studio-control-height: 40px');
    expect(quality).toContain('accent-color: var(--studio-accent)');
    expect(quality).toContain('min-block-size: 0');
  });
  it('reserves readable text, uncropped reference previews and compact-window detail navigation', () => {
    expect(quality).toContain('.v3-shell .library-layout[data-detail="open"]');
    expect(quality).toContain('@media (max-width: 1280px)');
    expect(quality).toContain('object-fit: contain');
    expect(quality).toContain('.v3-shell .story-entity-editor');
    expect(quality).toContain('font-size: var(--studio-font-sm)');
    expect(quality).toContain('prefers-reduced-motion: reduce');
  });
  it('maps legacy production panel fallbacks to the same dark semantic tokens', () => {
    expect(quality).toContain('--surface: var(--studio-surface-1)');
    expect(quality).toContain('--surface-raised: var(--studio-surface-2)');
    expect(quality).toContain('--border-subtle: var(--studio-border)');
    expect(quality).toContain('--text-muted: var(--studio-text-muted)');
    expect(quality).toContain('.scene-production-inline-success) { color: var(--studio-success)');
    expect(quality).toContain('.scene-production-inline-error) { color: var(--studio-danger)');
  });
});
