import { useCallback, useEffect, useState } from "react";
import { assetLibraryPage } from "../../services/tauriClient";
import type { AssetCategoryFilter, AssetView, PageCursor } from "../../types/asset";
import { AssetGrid } from "./AssetGrid";
import { AssetPreview } from "./AssetPreview";

const PROJECT_ID = "prj_default";
const categories: Array<{ value: AssetCategoryFilter; label: string }> = [
  { value: "ALL", label: "All" },
  { value: "SOURCE_IMAGE", label: "Source images" },
  { value: "GENERATED_IMAGE", label: "Generated images" },
];

export function AssetLibrary() {
  const [category, setCategory] = useState<AssetCategoryFilter>("ALL");
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [selectedAsset, setSelectedAsset] = useState<AssetView>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const load = useCallback(
    async (reset: boolean) => {
      setLoading(true);
      setError(undefined);
      try {
        const page = await assetLibraryPage(PROJECT_ID, category, reset ? undefined : cursor, 30);
        setAssets((current) =>
          reset
            ? page.items
            : [...current, ...page.items.filter((asset) => !current.some((item) => item.id === asset.id))],
        );
        setCursor(page.nextCursor);
      } catch (loadError: unknown) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      } finally {
        setLoading(false);
      }
    },
    [category, cursor],
  );

  useEffect(() => {
    setAssets([]);
    setCursor(undefined);
    void assetLibraryPage(PROJECT_ID, category, undefined, 30)
      .then((page) => {
        setAssets(page.items);
        setCursor(page.nextCursor);
      })
      .catch((loadError: unknown) => setError(loadError instanceof Error ? loadError.message : String(loadError)))
      .finally(() => setLoading(false));
  }, [category]);

  return (
    <section className="workspace-panel">
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Assets</span>
          <h2>Asset Library</h2>
          <p className="section-description">Browse source and generated images for this project.</p>
        </div>
        <button type="button" className="quiet-button" onClick={() => void load(true)} disabled={loading}>
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      </div>
      <div className="filter-row" aria-label="Asset categories">
        {categories.map((item) => (
          <button
            key={item.value}
            type="button"
            className={category === item.value ? "filter-button filter-button-active" : "filter-button"}
            onClick={() => setCategory(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {error && <p className="error-message">Unable to load assets: {error}</p>}
      <AssetGrid assets={assets} onSelect={setSelectedAsset} />
      {cursor && (
        <button type="button" className="load-more-button" onClick={() => void load(false)} disabled={loading}>
          {loading ? "Loading..." : "Load more"}
        </button>
      )}
      {selectedAsset && <AssetPreview asset={selectedAsset} onClose={() => setSelectedAsset(undefined)} />}
    </section>
  );
}
