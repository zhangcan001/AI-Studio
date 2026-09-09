// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetView, PageCursor } from "../../../types/asset";
import { useAssetVideoLibraryController } from "./useAssetVideoLibraryController";

const mocks = vi.hoisted(() => ({
  assetLibraryPage: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return { ...actual, assetLibraryPage: mocks.assetLibraryPage };
});

const cursor: PageCursor = { createdAt: "2026-09-09T00:00:00Z", id: "cursor-1" };

function asset(id: string): AssetView {
  return {
    id,
    assetType: "image",
    category: "source_image",
    name: id,
    originalName: `${id}.png`,
    mimeType: "image/png",
    width: 1344,
    height: 768,
    fileSize: 10,
    createdAt: "2026-09-09T00:00:00Z",
    thumbnailAvailable: true,
    isFavorite: false,
    tags: [],
  };
}

function page(items: AssetView[], nextCursor?: PageCursor) {
  return { items, nextCursor };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function options(overrides: Partial<Parameters<typeof useAssetVideoLibraryController>[0]> = {}) {
  return {
    projectId: "project-a",
    enabled: true,
    initialAssets: [],
    selectedAssetIds: new Set<string>(),
    ...overrides,
  };
}

describe("useAssetVideoLibraryController", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.assetLibraryPage.mockReset();
    mocks.assetLibraryPage.mockResolvedValue(page([]));
  });

  it("preserves the query contract and trims keyword after 300ms", async () => {
    const { result } = renderHook(() => useAssetVideoLibraryController(options()));

    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(1));
    expect(mocks.assetLibraryPage.mock.calls[0][0]).toEqual({
      projectId: "project-a",
      category: "ALL",
      keyword: undefined,
      mediaType: "ALL",
      sourceKind: "ALL",
      createdOrder: "NEWEST",
      cursor: undefined,
      limit: 30,
    });

    act(() => result.current.setKeywordInput(" cat "));
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(2), { timeout: 1000 });
    expect(mocks.assetLibraryPage.mock.calls[1][0].keyword).toBe("cat");
  });

  it("ignores stale success and stale error from an older request", async () => {
    const first = deferred<ReturnType<typeof page>>();
    const second = deferred<ReturnType<typeof page>>();
    mocks.assetLibraryPage.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
    const { result } = renderHook(() => useAssetVideoLibraryController(options()));

    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(1));
    act(() => result.current.setKeywordInput("new"));
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(2), { timeout: 1000 });

    await act(async () => { second.resolve(page([asset("new-result")])); });
    await waitFor(() => expect(result.current.availableAssets.map((item) => item.id)).toContain("new-result"));
    await act(async () => { first.reject(new Error("stale failure")); });
    expect(result.current.error).toBeUndefined();
    expect(result.current.availableAssets.map((item) => item.id)).not.toContain("old-result");
  });

  it("merges paginated assets by id and clears the cursor at the end", async () => {
    mocks.assetLibraryPage
      .mockResolvedValueOnce(page([asset("a"), asset("b")], cursor))
      .mockResolvedValueOnce(page([asset("b"), asset("c")]));
    const { result } = renderHook(() => useAssetVideoLibraryController(options()));

    await waitFor(() => expect(result.current.cursor).toEqual(cursor));
    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.cursor).toBeUndefined());
    expect(result.current.availableAssets.map((item) => item.id)).toEqual(["a", "b", "c"]);
  });

  it("preserves selected assets when a filtered page does not contain them", async () => {
    const selected = asset("selected");
    mocks.assetLibraryPage
      .mockResolvedValueOnce(page([selected], cursor))
      .mockResolvedValueOnce(page([asset("filtered")]));
    const { result } = renderHook(() => useAssetVideoLibraryController(options({
      initialAssets: [selected],
      selectedAssetIds: new Set([selected.id]),
    })));

    await waitFor(() => expect(result.current.cursor).toEqual(cursor));
    act(() => result.current.setKeywordInput("filtered"));
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(2), { timeout: 1000 });
    await waitFor(() => expect(result.current.availableAssets.map((item) => item.id)).toEqual(["selected", "filtered"]));
  });

  it("reports the full default page as authoritative", async () => {
    const onAuthoritativeAssetIds = vi.fn();
    const { result } = renderHook(() => useAssetVideoLibraryController(options({ onAuthoritativeAssetIds })));

    mocks.assetLibraryPage.mockClear();
    mocks.assetLibraryPage.mockResolvedValueOnce(page([asset("authoritative")]));
    act(() => result.current.refresh());
    await waitFor(() => expect(onAuthoritativeAssetIds).toHaveBeenCalledWith(["authoritative"]));
    expect(result.current.availableAssets.map((item) => item.id)).toEqual(["authoritative"]);
  });

  it("merges new initial assets without dropping loaded library assets", async () => {
    mocks.assetLibraryPage.mockResolvedValueOnce(page([asset("library" )], cursor));
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useAssetVideoLibraryController>[0]) => useAssetVideoLibraryController(props),
      { initialProps: options() },
    );

    await waitFor(() => expect(result.current.availableAssets.map((item) => item.id)).toEqual(["library"]));
    rerender(options({ initialAssets: [asset("initial")] }));
    await waitFor(() => expect(result.current.availableAssets.map((item) => item.id)).toEqual(["library", "initial"]));
  });

  it("does not fetch while disabled and fetches the current query when re-enabled", async () => {
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useAssetVideoLibraryController>[0]) => useAssetVideoLibraryController(props),
      { initialProps: options({ enabled: false }) },
    );
    expect(mocks.assetLibraryPage).not.toHaveBeenCalled();

    rerender(options({ enabled: true }));
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(1));
    expect(result.current.loading).toBe(false);
  });

  it("resets project query state and ignores the previous project's completion", async () => {
    const oldRequest = deferred<ReturnType<typeof page>>();
    mocks.assetLibraryPage.mockImplementationOnce(() => oldRequest.promise).mockResolvedValueOnce(page([asset("project-b")]));
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useAssetVideoLibraryController>[0]) => useAssetVideoLibraryController(props),
      { initialProps: options({ initialAssets: [asset("project-a")] }) },
    );

    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(1));
    rerender(options({ projectId: "project-b", initialAssets: [asset("project-b")] }));
    await waitFor(() => expect(result.current.availableAssets.map((item) => item.id)).toEqual(["project-b"]));
    await act(async () => { oldRequest.resolve(page([asset("stale-project-a")])); });
    expect(result.current.availableAssets.map((item) => item.id)).toEqual(["project-b"]);
    expect(result.current.keywordInput).toBe("");
    expect(result.current.mediaType).toBe("ALL");
  });
});
