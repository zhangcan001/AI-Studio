// @vitest-environment node
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { describe, expect, it } from 'vitest';

const create = readFileSync('src/features/create/CreatePage.css', 'utf8');
const runs = readFileSync('src/features/runs/RunsPage.css', 'utf8');
// Use Vite's existing CSS parser; do not add a direct dependency for a test.
const require = createRequire(import.meta.url);
const postcss = createRequire(require.resolve('vite'))('postcss');
function value(css: string, selector: string, property: string) {
  let found: string | undefined;
  postcss.parse(css).walkRules((rule: {selectors: string[]; nodes: {prop?: string; value?: string}[]}) => {
    if (rule.selectors.includes(selector)) for (const decl of rule.nodes) {
      if (decl.prop === property) found = decl.value;
    }
  });
  return found;
}

describe('charcoal and teal workbench refinement', () => {
  it('uses the shared surfaces and readable reference thumbnails in the mounted composer', () => {
    expect(value(create, '.v3-shell .create-composer-wrap .create-composer', 'background')).toBe('var(--studio-surface-1)');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-ref-thumb', 'width')).toBe('88px');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-ref-thumb', 'height')).toBe('88px');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-ref-thumb small', 'font-size')).toBe('var(--studio-font-xs)');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-editor:empty::before', 'color')).toBe('var(--studio-text-muted)');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-toolbar', 'flex-wrap')).toBe('wrap');
    expect(value(create, '.v3-shell .create-composer-wrap .composer-segment', 'height')).toBe('auto');
  });
  it('keeps storyboard selection and metadata consistent without changing the row size container', () => {
    expect(value(create, '.shot-row-card', 'background')).toBe('var(--studio-surface-1)');
    expect(value(create, '.shot-row-fields label span', 'color')).toBe('var(--studio-text-secondary)');
    expect(value(create, '.storyboard-shot-filters button[aria-pressed=true]', 'border-color')).toBe('var(--studio-accent)');
    expect(value(create, '.shot-row-bind-list', 'right')).toBe('0');
    expect(create).toMatch(/@container shot-rows \(min-width:900px\)/);
  });
  it('makes run selection stable and run cards readable in a narrow container, including a dialog', () => {
    expect(value(runs, '.run-list > button', 'border')).toBe('2px solid transparent');
    expect(value(runs, '.run-list > button[aria-pressed=true]', 'border-color')).toBe('var(--studio-accent)');
    expect(value(runs, '.run-results', 'grid-template-columns')).toBe('repeat(auto-fit,minmax(min(100%,200px),1fr))');
    expect(value(runs, '.runs-page', 'container-type')).toBe('inline-size');
    expect(runs).toContain('@container run-workspace (max-width: 680px)');
    expect(value(runs, '.runs-header p', 'max-width')).toBe('70ch');
    expect(value(runs, '.run-source', 'font-size')).toBe('var(--studio-font-xs)');
  });
  it('fits the five-column storyboard row at its actual 900px container threshold', () => {
    const grid = value(create, '.shot-row-card', 'grid-template-columns') ?? '';
    const widths = [...grid.matchAll(/(?:^| )(\d+)px|minmax\((\d+)px/g)].map(m => Number(m[1] ?? m[2]));
    expect(widths).toHaveLength(5);
    const gaps = 4 * Number.parseFloat(value(create, '.shot-row-card', 'gap') ?? '0');
    const padding = 2 * Number.parseFloat(value(create, '.shot-row-card', 'padding') ?? '0');
    const border = 2 * Number.parseFloat(value(create, '.shot-row-card', 'border') ?? '0');
    expect(widths.reduce((total, width) => total + width, 0) + gaps + padding + border).toBeLessThanOrEqual(900);
  });
});
