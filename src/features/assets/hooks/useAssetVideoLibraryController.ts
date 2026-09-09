import { useCallback, useEffect, useRef, useState } from "react";
import { assetLibraryPage } from "../../../services/tauriClient";
import type { AssetMediaTypeFilter, AssetView, PageCursor } from "../../../types/asset";
import { toUserMessage } from "../../../i18n/errorMessages";

export interface UseAssetVideoLibraryControllerOptions {
  projectId: string;
  enabled: boolean;
  initialAssets: AssetView[];
  selectedAssetIds: ReadonlySet<string>;
  onAuthoritativeAssetIds?: (assetIds: string[]) => void;
}

export function useAssetVideoLibraryController({
  projectId,
  enabled,
  initialAssets,
  selectedAssetIds,
  onAuthoritativeAssetIds,
}: UseAssetVideoLibraryControllerOptions) {
  const [availableAssets, setAvailableAssets] = useState<AssetView[]>(initialAssets);
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [mediaType, setMediaType] = useState<AssetMediaTypeFilter>("ALL");
  const [cursor, setCursor] = useState<PageCursor>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const requestVersion = useRef(0);
  const selectedIdsRef = useRef(selectedAssetIds);
  const initialAssetsRef = useRef(initialAssets);
  const onAuthoritativeAssetIdsRef = useRef(onAuthoritativeAssetIds);

  initialAssetsRef.current = initialAssets;

  useEffect(() => {
    selectedIdsRef.current = selectedAssetIds;
  }, [selectedAssetIds]);

  useEffect(() => {
    onAuthoritativeAssetIdsRef.current = onAuthoritativeAssetIds;
  }, [onAuthoritativeAssetIds]);

  useEffect(() => {
    setAvailableAssets((current) => {
      const byId = new Map(current.map((asset) => [asset.id, asset]));
      initialAssets.forEach((asset) => byId.set(asset.id, asset));
      if (byId.size === current.length) return current;
      return [...byId.values()];
    });
  }, [initialAssets]);

  useEffect(() => {
    requestVersion.current += 1;
    setAvailableAssets(initialAssets);
    setKeywordInput("");
    setKeyword("");
    setMediaType("ALL");
    setCursor(undefined);
    setLoading(false);
    setError(undefined);
  }, [projectId]);

  useEffect(() => {
    const timer = window.setTimeout(() => setKeyword(keywordInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [keywordInput]);

  const requestPage = useCallback(async (requestedCursor: PageCursor | undefined, reset: boolean) => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    try {
      const page = await assetLibraryPage({
        projectId,
        category: "ALL",
        keyword: keyword || undefined,
        mediaType,
        sourceKind: "ALL",
        createdOrder: "NEWEST",
        cursor: requestedCursor,
        limit: 30,
      });
      if (requestVersion.current !== version) return;
      const isAuthoritativeDefaultPage = reset && !keyword && mediaType === "ALL" && !page.nextCursor;
      setAvailableAssets((current) => {
        const base = reset
          ? [...initialAssetsRef.current, ...current.filter((asset) => selectedIdsRef.current.has(asset.id))]
          : current;
        const byId = new Map(base.map((asset) => [asset.id, asset]));
        page.items.forEach((asset) => byId.set(asset.id, asset));
        const merged = [...byId.values()];
        return isAuthoritativeDefaultPage ? page.items : merged;
      });
      setCursor(page.nextCursor);
      if (isAuthoritativeDefaultPage) {
        onAuthoritativeAssetIdsRef.current?.(page.items.map((asset) => asset.id));
      }
    } catch (loadError: unknown) {
      if (requestVersion.current === version) setError(toUserMessage(loadError));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [keyword, mediaType, projectId]);

  useEffect(() => {
    if (!enabled) return () => undefined;
    setCursor(undefined);
    void requestPage(undefined, true);
    return () => {
      requestVersion.current += 1;
    };
  }, [enabled, requestPage]);

  const refresh = useCallback(() => {
    if (!enabled) return;
    setCursor(undefined);
    void requestPage(undefined, true);
  }, [enabled, requestPage]);

  const loadMore = useCallback(() => {
    if (!enabled || !cursor || loading) return;
    void requestPage(cursor, false);
  }, [cursor, enabled, loading, requestPage]);

  return {
    availableAssets,
    keywordInput,
    setKeywordInput,
    mediaType,
    setMediaType,
    cursor,
    loading,
    error,
    loadMore,
    refresh,
  };
}
