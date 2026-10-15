import { useEffect, useMemo, useState } from "react";
import { productClient } from "../../product/client";
import type { LibraryItem, LibraryTag } from "../../product/libraryTypes";
import { AssetImageThumb } from "./CreateMediaPreview";

export type ImagePickTab = "all" | "character" | "scene" | "prop" | "untagged";
const TABS: { id: ImagePickTab; label: string; tagHints: string[] }[] = [
  { id: "all", label: "全部", tagHints: [] },
  { id: "character", label: "角色", tagHints: ["角色", "character", "char"] },
  { id: "scene", label: "场景", tagHints: ["场景", "scene", "环境"] },
  { id: "prop", label: "物品", tagHints: ["物品", "道具", "prop", "object"] },
  { id: "untagged", label: "未分类", tagHints: [] },
];

export type PickerAsset = { id: string; name: string; mediaKind: string };

function matchesHints(text: string, hints: string[]) {
  const lower = text.toLowerCase();
  return hints.some(h => lower.includes(h.toLowerCase()));
}

/** Thumbnail-grid image picker (~112px tiles): library imageGet thumbs + local choices, search, entity tabs, import. */
export function ImageAssetPicker({
  projectId, open, title = "选择图片素材", excludeIds = [], localChoices = [],
  onSelect, onClose, onImportFile, onImportFolder, onOpenLibrary,
}: {
  projectId: string;
  open: boolean;
  title?: string;
  excludeIds?: string[];
  localChoices?: PickerAsset[];
  onSelect: (assetId: string) => void;
  onClose: () => void;
  onImportFile?: () => void;
  onImportFolder?: () => void;
  onOpenLibrary?: () => void;
}) {
  const [tab, setTab] = useState<ImagePickTab>("all");
  const [keyword, setKeyword] = useState("");
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [tags, setTags] = useState<LibraryTag[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open) return;
    setTab("all"); setKeyword(""); setError(undefined);
  }, [open, projectId]);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setBusy(true); setError(undefined);
    const hints = TABS.find(t => t.id === tab)?.tagHints ?? [];
    const tagsFn = productClient.library.tagsList;
    const listFn = productClient.library.list;
    void (typeof tagsFn === "function" ? tagsFn(projectId) : Promise.resolve([] as LibraryTag[]))
      .catch(() => [] as LibraryTag[])
      .then(async tagList => {
      if (!alive) return;
      setTags(tagList);
      const tagId = (tab === "character" || tab === "scene" || tab === "prop")
        ? (tagList.find(tag => matchesHints(tag.name, hints))?.id ?? null)
        : null;
      try {
        if (typeof listFn !== "function") {
          if (alive) { setItems([]); setBusy(false); }
          return;
        }
        const page = await listFn(projectId, {
          category: "images", keyword: keyword.trim() || null, favoriteOnly: null,
          tagId, sourceKind: null, cursor: null, limit: 100,
        });
        if (!alive) return;
        setItems(page.items.filter(i => i.resourceRef.kind === "asset"));
      } catch (cause) {
        if (alive) {
          setItems([]);
          setError(cause instanceof Error ? cause.message : "无法读取图片素材");
        }
      } finally {
        if (alive) setBusy(false);
      }
    });
    return () => { alive = false; };
  }, [open, projectId, tab, keyword]);

  const entityHints = useMemo(() => TABS.filter(t => t.tagHints.length).flatMap(t => t.tagHints), []);

  const libraryRows = useMemo(() => {
    const exclude = new Set(excludeIds);
    let rows = items.filter(i => i.resourceRef.kind === "asset" && !exclude.has(i.resourceRef.id));
    // When tag filter is unavailable, fall back to title/subtype heuristics for entity tabs.
    if (tab === "character" || tab === "scene" || tab === "prop") {
      const hints = TABS.find(x => x.id === tab)!.tagHints;
      const hasTag = tags.some(tag => matchesHints(tag.name, hints));
      if (!hasTag) rows = rows.filter(i => matchesHints(i.title, hints) || matchesHints(i.subtype, hints));
    } else if (tab === "untagged") {
      rows = rows.filter(i => !matchesHints(i.title, entityHints) && !matchesHints(i.subtype, entityHints));
    }
    return rows;
  }, [items, excludeIds, tab, tags, entityHints]);

  const localRows = useMemo(() => {
    const q = keyword.trim().toLowerCase();
    const exclude = new Set(excludeIds);
    const libIds = new Set(
      items.filter(i => i.resourceRef.kind === "asset").map(i => (i.resourceRef.kind === "asset" ? i.resourceRef.id : "")),
    );
    let rows = localChoices.filter(a => a.mediaKind === "image" && !exclude.has(a.id) && !libIds.has(a.id));
    if (q) rows = rows.filter(a => a.name.toLowerCase().includes(q));
    if (tab === "character" || tab === "scene" || tab === "prop") {
      const hints = TABS.find(x => x.id === tab)!.tagHints;
      rows = rows.filter(a => matchesHints(a.name, hints));
    } else if (tab === "untagged") {
      rows = rows.filter(a => !matchesHints(a.name, entityHints));
    }
    return rows;
  }, [localChoices, keyword, excludeIds, items, tab, entityHints]);

  if (!open) return null;
  return <div className="image-asset-picker-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-label={title} className="image-asset-picker" onClick={e => e.stopPropagation()}>
      <header><strong>{title}</strong><button type="button" onClick={onClose}>关闭</button></header>
      <div className="image-asset-picker-toolbar">
        <input aria-label="搜索图片" placeholder="搜索名称…" value={keyword} onChange={e => setKeyword(e.target.value)} />
        <div className="image-asset-picker-tabs" role="tablist" aria-label="素材分类">
          {TABS.map(t => <button key={t.id} type="button" role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>)}
        </div>
      </div>
      {error && <p role="status" className="image-asset-picker-status">资源库暂不可用：{error}（仍可选用本页已有素材）</p>}
      {busy && <p role="status">正在加载图片…</p>}
      {!busy && !libraryRows.length && !localRows.length && <p className="image-asset-picker-empty">没有可选用的图片。可从电脑导入，或先在资源库添加素材。</p>}
      <div className="image-asset-picker-grid" role="listbox" aria-label="图片网格">
        {localRows.map(a => <button key={`local-${a.id}`} type="button" role="option" aria-selected="false" className="image-asset-tile" onClick={() => { onSelect(a.id); onClose(); }}>
          <AssetImageThumb projectId={projectId} assetId={a.id} name={a.name} size="lg" interactive={false} />
          <span className="image-asset-tile-name">{a.name}</span>
        </button>)}
        {libraryRows.map(item => {
          const id = item.resourceRef.kind === "asset" ? item.resourceRef.id : "";
          return <button key={id} type="button" role="option" aria-selected="false" className="image-asset-tile" onClick={() => { onSelect(id); onClose(); }}>
            <AssetImageThumb projectId={projectId} assetId={id} name={item.title} size="lg" interactive={false} />
            <span className="image-asset-tile-name">{item.title}</span>
          </button>;
        })}
      </div>
      <footer className="image-asset-picker-actions">
        {onImportFile && <button type="button" onClick={() => { onClose(); onImportFile(); }}>从电脑导入…</button>}
        {onImportFolder && <button type="button" onClick={() => { onClose(); onImportFolder(); }}>导入图片文件夹…</button>}
        {onOpenLibrary && <button type="button" onClick={() => { onClose(); onOpenLibrary(); }}>在资源库查找</button>}
      </footer>
    </section>
  </div>;
}
