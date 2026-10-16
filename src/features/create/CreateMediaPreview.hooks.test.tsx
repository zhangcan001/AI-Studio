// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { presetAspectKey, ResolutionPicker } from "./CreateInputs";
import { loadLibraryImageUrl, retainLibraryImage, useLibraryImageUrl } from "./CreateMediaPreview";
import type { CreateController } from "./CreateController";

const imageGet = vi.fn();
vi.mock("../../product/client", () => ({
  productClient: {
    creation: { mediaUrl: () => "media://x" },
    library: { imageGet: (...a: unknown[]) => imageGet(...a) },
  },
}));

afterEach(() => { cleanup(); imageGet.mockReset(); });

it("audit: an evicted pending image revokes its eventual orphan URL", async () => {
  let finish!: (bytes: number[]) => void;
  imageGet.mockImplementation((_p: string, ref: { id: string }) => ref.id === "slow" ? new Promise<number[]>(yes => { finish = yes; }) : Promise.resolve([1]));
  let n = 0; URL.createObjectURL = vi.fn(() => `blob:orphan-${++n}`); URL.revokeObjectURL = vi.fn();
  const old = loadLibraryImageUrl("audit-orphan", "slow");
  await Promise.resolve();
  await Promise.all(Array.from({ length: 40 }, (_, i) => loadLibraryImageUrl("audit-orphan", `filler${i}`)));
  finish([2]);
  const result = await old;
  expect(result).toBeUndefined();
  expect(URL.revokeObjectURL).toHaveBeenCalledWith(`blob:orphan-${n}`);
});

it("audit: a pinned oldest image does not block eviction of unreferenced images", async () => {
  imageGet.mockResolvedValue([1]);
  let n = 0; URL.createObjectURL = vi.fn(() => `blob:pinned-${++n}`); URL.revokeObjectURL = vi.fn();
  const release = retainLibraryImage("audit-pinned", "held");
  const held = await loadLibraryImageUrl("audit-pinned", "held");
  const oldestFree = await loadLibraryImageUrl("audit-pinned", "free0");
  for (let i = 1; i <= 31; i++) await loadLibraryImageUrl("audit-pinned", `free${i}`);
  try {
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(oldestFree);
    expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(held);
  } finally { release(); }
});

it("preserves every active lease even above the cache cap and releases idempotently", async () => {
  imageGet.mockResolvedValue([1]);
  let n = 0; URL.createObjectURL = vi.fn(() => `blob:active-${++n}`); URL.revokeObjectURL = vi.fn();
  const release = Array.from({ length: 40 }, (_, i) => retainLibraryImage("audit-active", `${i}`));
  try {
    const urls = await Promise.all(release.map((_, i) => loadLibraryImageUrl("audit-active", `${i}`)));
    for (const url of urls) { expect(url).toBeTruthy(); expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(url); }
    release[0](); release[0]();
    expect(URL.revokeObjectURL).toHaveBeenCalledWith(urls[0]);
    expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(urls[1]);
  } finally { release.forEach(fn => fn()); }
});

it("a rejected evicted load cannot delete a newer same-key cache entry", async () => {
  let reject!: (reason: Error) => void;
  imageGet.mockImplementation((_p, ref) => ref.id === "replaced" ? new Promise((_yes, no) => { reject = no; }) : Promise.resolve([1]));
  let n = 0; URL.createObjectURL = vi.fn(() => `blob:replace-${++n}`); URL.revokeObjectURL = vi.fn();
  const old = loadLibraryImageUrl("audit-replaced", "replaced");
  await Promise.resolve();
  await Promise.all(Array.from({ length: 40 }, (_, i) => loadLibraryImageUrl("audit-replaced", `filler${i}`)));
  imageGet.mockResolvedValue([2]);
  const release = retainLibraryImage("audit-replaced", "replaced");
  try {
    const current = await loadLibraryImageUrl("audit-replaced", "replaced");
    reject(new Error("old read failed"));
    await old;
    expect(await loadLibraryImageUrl("audit-replaced", "replaced")).toBe(current);
    expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(current);
  } finally { release(); }
});

it("presetAspectKey reads nominal aspect from backend mp ids, not rounded pixels", () => {
  expect(presetAspectKey({ id: "mp-1_0-16x9", width: 1376, height: 768 })).toBe("16:9");
  expect(presetAspectKey({ id: "mp-0_2-16x9", width: 608, height: 352 })).toBe("16:9");
  expect(presetAspectKey({ id: "mp-0_98-9x16", width: 768, height: 1344 })).toBe("9:16");
});

it("resolution menu keeps every same-aspect megapixel preset including 1376x768", () => {
  const presets = [
    { id: "mp-0_98-16x9", label: "0.98MP · 1344×768", width: 1344, height: 768 },
    { id: "mp-1_0-16x9", label: "1.0MP · 1376×768", width: 1376, height: 768 },
    { id: "mp-0_8-16x9", label: "0.8MP · 1216×672", width: 1216, height: 672 },
    { id: "mp-0_2-16x9", label: "0.2MP · 608×352", width: 608, height: 352 },
    { id: "mp-0_98-9x16", label: "0.98MP · 768×1344", width: 768, height: 1344 },
  ];
  const c = { busy: false, chooseResolution: vi.fn() } as unknown as CreateController;
  render(<ResolutionPicker controller={c} presets={presets} current={presets[0]} />);
  const options = [...screen.getByLabelText("视频分辨率").querySelectorAll("option")].map(o => o.textContent);
  expect(options).toEqual(expect.arrayContaining(["0.98MP · 1344×768", "1.0MP · 1376×768", "0.8MP · 1216×672", "0.2MP · 608×352"]));
  expect(options.some(t => t?.includes("768×1344"))).toBe(false);
});

function Probe({ projectId, assetId }: { projectId: string; assetId: string }) {
  const url = useLibraryImageUrl(projectId, assetId);
  return <span data-testid="url">{url ?? "loading"}</span>;
}

it("clears the previous image URL immediately when the asset identity changes", async () => {
  imageGet.mockImplementation((_p: string, ref: { id: string }) => {
    if (ref.id === "a") return Promise.resolve([1, 2, 3]);
    // Never resolve B in this assertion — we only require the stale A URL to clear.
    return new Promise<number[]>(() => {});
  });
  URL.createObjectURL = vi.fn(() => "blob:img-a");
  URL.revokeObjectURL = vi.fn();
  const view = render(<Probe projectId="p" assetId="a" />);
  await waitFor(() => expect(screen.getByTestId("url").textContent).toBe("blob:img-a"));
  view.rerender(<Probe projectId="p" assetId="b" />);
  expect(screen.getByTestId("url").textContent).toBe("loading");
});
