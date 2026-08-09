import { useCallback, useEffect, useRef, useState } from "react";
import { assetLibraryPage } from "../../services/tauriClient";
import type { AssetCategoryFilter, AssetView, PageCursor } from "../../types/asset";
import { toUserMessage } from "../../i18n/errorMessages";
import { AssetGrid } from "./AssetGrid";
import { AssetPreview } from "./AssetPreview";

const categories: Array<{ value: AssetCategoryFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "SOURCE_IMAGE", label: "源图片" },
  { value: "SOURCE_VIDEO", label: "源视频" },
  { value: "SOURCE_AUDIO", label: "源音频" },
  { value: "GENERATED_IMAGE", label: "生成图片" },
  { value: "GENERATED_VIDEO", label: "生成视频" },
];

export function AssetLibrary({ projectId }: { projectId: string }) {
  const [category, setCategory] = useState<AssetCategoryFilter>("ALL");
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [selectedAsset, setSelectedAsset] = useState<AssetView>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const requestVersion = useRef(0);

  const load = useCallback(
    async (reset: boolean) => {
      const version = ++requestVersion.current;
      const requestedCursor = reset ? undefined : cursor;
      setLoading(true);
      setError(undefined);
      try {
        const page = await assetLibraryPage(projectId, category, requestedCursor, 30);
        if (requestVersion.current !== version) return;
        setAssets((current) =>
          reset
            ? page.items
            : [...current, ...page.items.filter((asset) => !current.some((item) => item.id === asset.id))],
        );
        setCursor(page.nextCursor);
      } catch (loadError: unknown) {
        if (requestVersion.current === version) {
          setError(toUserMessage(loadError));
        }
      } finally {
        if (requestVersion.current === version) setLoading(false);
      }
    },
    [category, cursor, projectId],
  );

  useEffect(() => {
    const version = ++requestVersion.current;
    setAssets([]);
    setCursor(undefined);
    setSelectedAsset(undefined);
    setError(undefined);
    setLoading(true);
    void assetLibraryPage(projectId, category, undefined, 30)
      .then((page) => {
        if (requestVersion.current !== version) return;
        setAssets(page.items);
        setCursor(page.nextCursor);
      })
      .catch((loadError: unknown) => {
        if (requestVersion.current === version) {
          setError(toUserMessage(loadError));
        }
      })
      .finally(() => {
        if (requestVersion.current === version) setLoading(false);
      });
    return () => {
      requestVersion.current += 1;
    };
  }, [category, projectId]);

  return (
    <section className="workspace-panel" aria-busy={loading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">资产</span>
          <h2>资产库</h2>
          <p className="section-description">浏览当前项目的源素材和生成结果。</p>
        </div>
        <button type="button" className="quiet-button" onClick={() => void load(true)} disabled={loading}>
          {loading ? "正在刷新..." : "刷新"}
        </button>
      </div>
      <div className="filter-row" aria-label="资产分类">
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
      {error && <p className="error-message">资产加载失败：{error}</p>}
      <AssetGrid projectId={projectId} assets={assets} onSelect={setSelectedAsset} />
      {cursor && (
        <button type="button" className="load-more-button" onClick={() => void load(false)} disabled={loading}>
          {loading ? "正在加载..." : "加载更多"}
        </button>
      )}
      {selectedAsset && (
        <AssetPreview projectId={projectId} asset={selectedAsset} onClose={() => setSelectedAsset(undefined)} />
      )}
    </section>
  );
}
