import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const json = (path: string) => JSON.parse(readFileSync(path, 'utf8'));

describe('personal release Windows bundle compatibility', () => {
  it('keeps the product version aligned without discarding the personal label', () => {
    const config = json('src-tauri/tauri.conf.json');
    const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8');
    expect(config.version).toBe(json('package.json').version);
    expect(cargo.match(/^version = "([^"]+)"/m)?.[1]).toBe(config.version);
    expect(config.version).toBe('2.1.0-personal');
  });

  it('supplies a numeric MSI version instead of deriving a nonnumeric prerelease', () => {
    const config = json('src-tauri/tauri.conf.json');
    const version = config.bundle.windows?.wix?.version;
    expect(version).toMatch(/^\d+\.\d+\.\d+(?:\.\d+)?$/);
    expect(version.split('.').slice(0, 3).join('.')).toBe(config.version.split('-')[0]);
    const parts = version.split('.').map(Number);
    expect(parts[0]).toBeLessThanOrEqual(255);
    expect(parts[1]).toBeLessThanOrEqual(255);
    expect(parts.slice(2).every((part: number) => part <= 65535)).toBe(true);
    const schema = json('node_modules/@tauri-apps/cli/config.schema.json');
    expect(schema.definitions.WixConfig.properties.version.type).toContain('string');
  });

  it('retains both Windows bundle targets and the installed application identity', () => {
    const config = json('src-tauri/tauri.conf.json');
    expect(config.bundle.active).toBe(true);
    expect(config.bundle.targets).toBe('all');
    expect(config.identifier).toBe('com.aistudio.desktop');
    expect(config.productName).toBe('AI Studio');
  });
});
