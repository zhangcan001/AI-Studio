import { useEffect, useMemo, useState } from "react";
import {
  assignProductionSceneShots,
  createProductionEpisode,
  createProductionScene,
  createProductionSeries,
  deleteProductionEpisode,
  deleteProductionScene,
  deleteProductionSeries,
  listProductionStructure,
  reorderProductionEpisodes,
  reorderProductionSceneShots,
  reorderProductionScenes,
  reorderProductionSeries,
  unassignProductionSceneShots,
  updateProductionEpisode,
  updateProductionScene,
  updateProductionSeries,
} from "../../services/tauriClient";
import type { ShotView } from "../../types/shot";
import type {
  ProductionEpisode,
  ProductionScene,
  ProductionSeries,
  ProductionStructureTree,
} from "../../types/productionStructure";
import { toUserMessage } from "../../i18n/errorMessages";
import {
  normalizeStructureName,
  orderedEpisodes,
  orderedScenes,
  orderedSeries,
  moveOrderedId,
  productionSceneOptions,
  shotSceneIndex,
} from "./productionStructureState";
import "./ProductionStructurePanel.css";

interface Props {
  projectId: string;
  tree: ProductionStructureTree;
  shots: ShotView[];
  selectedShotId?: string;
  onSelectShot: (shotId: string) => void;
  onChanged: (tree: ProductionStructureTree) => void;
  onError?: (message: string) => void;
}

type StructureNodeKind = "series" | "episode" | "scene";

export function ProductionStructurePanel({ projectId, tree, shots, selectedShotId, onSelectShot, onChanged, onError }: Props) {
  const [busy, setBusy] = useState(false);
  const [search, setSearch] = useState("");
  const [selectedSceneId, setSelectedSceneId] = useState("");
  const [selectedShotIds, setSelectedShotIds] = useState<string[]>([]);
  const shotIndex = useMemo(() => shotSceneIndex(tree), [tree]);
  const shotById = useMemo(() => new Map(shots.map((shot) => [shot.id, shot])), [shots]);
  const sceneOptions = useMemo(() => productionSceneOptions(tree).filter((option) => option.value !== "ALL" && option.value !== "UNASSIGNED"), [tree]);
  const searchableShots = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return [...shots]
      .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id))
      .filter((shot) => !query || `${shot.name}\n${shot.promptText}`.toLocaleLowerCase().includes(query))
      .slice(0, 500);
  }, [search, shots]);
  const selectedShot = selectedShotId ? shotById.get(selectedShotId) : undefined;

  useEffect(() => {
    if (selectedSceneId && sceneOptions.some((option) => option.value === selectedSceneId)) return;
    setSelectedSceneId(sceneOptions[0]?.value ?? "");
  }, [sceneOptions, selectedSceneId]);

  useEffect(() => {
    setSelectedShotIds((current) => current.filter((shotId) => shotById.has(shotId)));
  }, [shotById]);

  async function refresh() {
    onChanged(await listProductionStructure(projectId));
  }

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    onError?.("");
    try {
      await action();
      await refresh();
    } catch (error: unknown) {
      onError?.(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function promptName(label: string): string | undefined {
    return normalizeStructureName(window.prompt(`${label}名称`, "" ) ?? "");
  }

  async function addSeries() {
    const name = promptName("系列");
    if (name) await run(() => createProductionSeries({ projectId, name }));
  }

  async function addEpisode(series: ProductionSeries) {
    const name = promptName("第集");
    if (name) await run(() => createProductionEpisode({ projectId, seriesId: series.id, name }));
  }

  async function addScene(episode: ProductionEpisode) {
    const name = promptName("场景");
    if (name) await run(() => createProductionScene({ projectId, episodeId: episode.id, name }));
  }

  async function renameNode(kind: StructureNodeKind, item: ProductionSeries | ProductionEpisode | ProductionScene) {
    const name = promptName("新");
    if (!name) return;
    if (kind === "series") {
      const series = item as ProductionSeries;
      await run(() => updateProductionSeries({ projectId, seriesId: series.id, name, description: series.description }));
    } else if (kind === "episode") {
      const episode = item as ProductionEpisode;
      await run(() => updateProductionEpisode({ projectId, episodeId: episode.id, name, description: episode.description }));
    } else {
      const scene = item as ProductionScene;
      await run(() => updateProductionScene({ projectId, sceneId: scene.id, name, description: scene.description }));
    }
  }

  async function deleteNode(kind: StructureNodeKind, item: ProductionSeries | ProductionEpisode | ProductionScene) {
    const name = item.name;
    const shotsInNode = kind === "scene"
      ? (item as ProductionScene).shotIds.length
      : kind === "episode"
        ? (item as ProductionEpisode).scenes.reduce((count, scene) => count + scene.shotIds.length, 0)
        : (item as ProductionSeries).episodes.reduce((count, episode) => count + episode.scenes.reduce((sceneCount, scene) => sceneCount + scene.shotIds.length, 0), 0);
    const scope = kind === "scene" ? "场景" : kind === "episode" ? "集" : "系列";
    const message = `删除${scope}“${name}”？${shotsInNode ? `其中 ${shotsInNode} 个镜头不会删除，将移动到未归档。` : "镜头不会删除，将移动到未归档。"}`;
    if (!window.confirm(message)) return;
    if (kind === "series") await run(() => deleteProductionSeries(projectId, item.id));
    else if (kind === "episode") await run(() => deleteProductionEpisode(projectId, item.id));
    else await run(() => deleteProductionScene(projectId, item.id));
  }

  async function moveSeries(series: ProductionSeries, delta: -1 | 1) {
    await run(() => reorderProductionSeries(projectId, moveOrderedId(orderedSeries(tree).map((item) => item.id), series.ordinal, delta)));
  }

  async function moveEpisode(series: ProductionSeries, episode: ProductionEpisode, delta: -1 | 1) {
    await run(() => reorderProductionEpisodes(projectId, series.id, moveOrderedId(orderedEpisodes(series).map((item) => item.id), episode.ordinal, delta)));
  }

  async function moveScene(episode: ProductionEpisode, scene: ProductionScene, delta: -1 | 1) {
    await run(() => reorderProductionScenes(projectId, episode.id, moveOrderedId(orderedScenes(episode).map((item) => item.id), scene.ordinal, delta)));
  }

  async function moveSceneShot(scene: ProductionScene, index: number, delta: -1 | 1) {
    await run(() => reorderProductionSceneShots({
      projectId,
      sceneId: scene.id,
      orderedShotIds: moveOrderedId(scene.shotIds, index, delta),
    }));
  }

  async function assignSelectedShots() {
    if (!selectedSceneId || !selectedShotIds.length) return;
    await run(() => assignProductionSceneShots({ projectId, sceneId: selectedSceneId, shotIds: selectedShotIds.slice(0, 500) }));
    setSelectedShotIds([]);
  }

  async function unassignSelectedShots() {
    if (!selectedShotIds.length) return;
    await run(() => unassignProductionSceneShots({ projectId, shotIds: selectedShotIds.slice(0, 500) }));
    setSelectedShotIds([]);
  }

  function toggleShot(shotId: string) {
    setSelectedShotIds((current) => current.includes(shotId) ? current.filter((id) => id !== shotId) : [...current, shotId].slice(0, 500));
  }

  function toggleAllVisible() {
    const visibleIds = searchableShots.map((shot) => shot.id);
    const allSelected = visibleIds.length > 0 && visibleIds.every((id) => selectedShotIds.includes(id));
    setSelectedShotIds((current) => allSelected ? current.filter((id) => !visibleIds.includes(id)) : [...new Set([...current, ...visibleIds])].slice(0, 500));
  }

  const actions = (kind: StructureNodeKind, item: ProductionSeries | ProductionEpisode | ProductionScene, index: number, count: number, onMove: (delta: -1 | 1) => void) => (
    <span className="production-structure-actions">
      <button type="button" className="quiet-button" onClick={() => void renameNode(kind, item)} disabled={busy}>重命名</button>
      <button type="button" className="quiet-button" onClick={() => void onMove(-1)} disabled={busy || index === 0} aria-label="上移">↑</button>
      <button type="button" className="quiet-button" onClick={() => void onMove(1)} disabled={busy || index === count - 1} aria-label="下移">↓</button>
      <button type="button" className="quiet-button danger-button" onClick={() => void deleteNode(kind, item)} disabled={busy}>删除</button>
    </span>
  );

  return (
    <section className="production-structure-panel" aria-labelledby="production-structure-title" aria-busy={busy}>
      <div className="production-structure-heading">
        <div><span className="section-label">Production structure</span><h3 id="production-structure-title">内容结构</h3><p className="section-description">Project → Series → Episode → Scene；删除结构不会删除镜头。</p></div>
        <button type="button" onClick={() => void addSeries()} disabled={busy}>新增系列</button>
      </div>
      <div className="production-structure-tree" role="tree" aria-label="项目内容结构">
        {orderedSeries(tree).map((series, seriesIndex, seriesItems) => <div key={series.id} className="production-structure-series" role="treeitem" aria-expanded="true">
          <div className="production-structure-node production-structure-series-node"><strong>系列 {String(series.ordinal + 1).padStart(2, "0")} · {series.name}</strong>{actions("series", series, seriesIndex, seriesItems.length, (delta) => void moveSeries(series, delta))}<button type="button" className="quiet-button" onClick={() => void addEpisode(series)} disabled={busy}>新增集</button></div>
          <div className="production-structure-children">
            {orderedEpisodes(series).map((episode, episodeIndex, episodeItems) => <div key={episode.id} className="production-structure-episode" role="treeitem" aria-expanded="true">
              <div className="production-structure-node"><strong>第 {String(episode.ordinal + 1).padStart(2, "0")} 集 · {episode.name}</strong>{actions("episode", episode, episodeIndex, episodeItems.length, (delta) => void moveEpisode(series, episode, delta))}<button type="button" className="quiet-button" onClick={() => void addScene(episode)} disabled={busy}>新增场景</button></div>
              <div className="production-structure-children">
                {orderedScenes(episode).map((scene, sceneIndex, sceneItems) => <div key={scene.id} className={`production-structure-scene${selectedSceneId === scene.id ? " production-structure-scene-selected" : ""}`} role="treeitem" aria-expanded="true">
                  <div className="production-structure-node"><button type="button" className="production-structure-title-button" onClick={() => setSelectedSceneId(scene.id)}><strong>场景 {String(scene.ordinal + 1).padStart(2, "0")} · {scene.name}</strong><small>{scene.shotIds.length} 个镜头</small></button>{actions("scene", scene, sceneIndex, sceneItems.length, (delta) => void moveScene(episode, scene, delta))}</div>
                  {scene.shotIds.length > 0 && <div className="production-structure-shot-list">{scene.shotIds.map((shotId, shotIndex) => <div key={shotId} className="production-structure-shot-row"><button type="button" className="production-structure-shot-name" onClick={() => { onSelectShot(shotId); setSelectedShotIds([shotId]); }}>{shotById.get(shotId)?.name ?? shotId}</button><button type="button" className="quiet-button" onClick={() => void moveSceneShot(scene, shotIndex, -1)} disabled={busy || shotIndex === 0} aria-label={`上移 ${shotById.get(shotId)?.name ?? shotId}`}>↑</button><button type="button" className="quiet-button" onClick={() => void moveSceneShot(scene, shotIndex, 1)} disabled={busy || shotIndex === scene.shotIds.length - 1} aria-label={`下移 ${shotById.get(shotId)?.name ?? shotId}`}>↓</button></div>)}</div>}
                </div>)}
                {!episodeItems.length && <p className="production-structure-empty">暂无场景。</p>}
              </div>
            </div>)}
            {!series.episodes.length && <p className="production-structure-empty">暂无集。</p>}
          </div>
        </div>)}
        {!tree.series.length && <p className="production-structure-empty">暂无内容结构，先新增一个系列。</p>}
      </div>
      <div className="production-structure-assignment">
        <div className="production-structure-assignment-heading"><div><strong>镜头归属</strong><small>{selectedShot ? `当前镜头：${selectedShot.name}` : "勾选 1～500 个镜头后批量分配"}</small></div><span>{selectedShotIds.length} 已选</span></div>
        <div className="production-structure-assignment-controls">
          <label><span className="sr-only">目标场景</span><select value={selectedSceneId} onChange={(event) => setSelectedSceneId(event.target.value)} disabled={busy || !sceneOptions.length} aria-label="目标场景"><option value="">选择场景</option>{sceneOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
          <button type="button" onClick={() => void assignSelectedShots()} disabled={busy || !selectedSceneId || !selectedShotIds.length}>分配到场景</button>
          <button type="button" className="quiet-button" onClick={() => void unassignSelectedShots()} disabled={busy || !selectedShotIds.length}>取消所属场景</button>
        </div>
        <div className="production-structure-shot-picker">
          <div className="production-structure-picker-toolbar"><input type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索镜头名称或 Prompt" aria-label="搜索待分配镜头" /><button type="button" className="quiet-button" onClick={toggleAllVisible} disabled={busy || !searchableShots.length}>{searchableShots.length > 0 && searchableShots.every((shot) => selectedShotIds.includes(shot.id)) ? "取消全选" : "全选当前结果"}</button></div>
          <div className="production-structure-picker-list">{searchableShots.map((shot) => <label key={shot.id} className="production-structure-picker-row"><input type="checkbox" checked={selectedShotIds.includes(shot.id)} onChange={() => toggleShot(shot.id)} disabled={busy} /><span><strong>{String(shot.ordinal + 1).padStart(2, "0")} · {shot.name}</strong><small>{shotIndex[shot.id] ? `已归档 · ${shotIndex[shot.id]}` : "未归档"}</small></span></label>)}{!searchableShots.length && <p className="production-structure-empty">没有匹配的镜头。</p>}</div>
        </div>
      </div>
    </section>
  );
}
