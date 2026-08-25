import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type {
  ProductionEpisode,
  ProductionScene,
  ProductionSeries,
  ProductionStructureTree,
} from "../../types/productionStructure";
import type { ProjectView } from "../../types/project";
import type { ShotView } from "../../types/shot";
import {
  isSameWorkspaceSelection,
  workspaceSelectionKey,
  type WorkspaceSelection,
} from "../../types/workspaceSelection";
import { orderedEpisodes, orderedScenes, orderedSeries } from "./productionStructureState";
import "./ProjectStructureTree.css";

export type ProjectStructureCreateTarget = "shot" | "series" | "episode" | "scene";

export interface ProjectStructureTreeProps {
  project: Pick<ProjectView, "id" | "name">;
  tree: ProductionStructureTree;
  shots: readonly ShotView[];
  selectedSelection?: WorkspaceSelection;
  onSelectSelection: (selection: WorkspaceSelection) => void;
  /** Header + menu hook for the existing create/import/structure CRUD flows. */
  onCreate?: (target: ProjectStructureCreateTarget, context: WorkspaceSelection) => void;
  /** Opens the existing structure-management surface; CRUD remains outside this tree. */
  openManagement?: (context: WorkspaceSelection) => void;
  /** Extra header actions, such as bulk import, manifest export, or a management drawer. */
  headerActions?: ReactNode;
  /** Per-node action slot for rename/delete/reorder/assignment controls. */
  renderNodeActions?: (selection: WorkspaceSelection) => ReactNode;
  /** Optional host-owned search/status filter; the tree never fetches or mutates. */
  shotFilter?: (shot: ShotView) => boolean;
  /** Bounds DOM work when the currently selected scene contains many shots. */
  maxVisibleShots?: number;
}

export const DEFAULT_MAX_VISIBLE_SHOTS = 120;

type TreeItemKind = WorkspaceSelection["type"];

interface TreeItemOptions {
  nodeKey: string;
  selection: WorkspaceSelection;
  kind: TreeItemKind;
  label: string;
  meta?: string;
  level: number;
  position: number;
  setSize: number;
  hasChildren: boolean;
  expanded?: boolean;
  onToggle?: () => void;
  children?: ReactNode;
}

interface NavigationModel {
  keys: string[];
  parentByKey: Map<string, string | undefined>;
}

export function limitVisibleShotIds(
  shotIds: readonly string[],
  selectedShotId: string | undefined,
  maxVisibleShots = DEFAULT_MAX_VISIBLE_SHOTS,
): string[] {
  const limit = Number.isFinite(maxVisibleShots) ? Math.max(1, Math.floor(maxVisibleShots)) : DEFAULT_MAX_VISIBLE_SHOTS;
  const uniqueIds = [...new Set(shotIds)];
  const visibleIds = uniqueIds.slice(0, limit);
  if (!selectedShotId || !uniqueIds.includes(selectedShotId) || visibleIds.includes(selectedShotId)) return visibleIds;
  return [...visibleIds.slice(0, Math.max(0, limit - 1)), selectedShotId];
}

function selectionForNode(kind: TreeItemKind, id: string): WorkspaceSelection {
  switch (kind) {
    case "project":
      return { type: "project", projectId: id };
    case "series":
      return { type: "series", seriesId: id };
    case "episode":
      return { type: "episode", episodeId: id };
    case "scene":
      return { type: "scene", sceneId: id };
    case "shot":
      return { type: "shot", shotId: id };
  }
}

function nodeKey(kind: TreeItemKind, id: string): string {
  return workspaceSelectionKey(selectionForNode(kind, id));
}

function toggleId(current: Set<string>, id: string): Set<string> {
  const next = new Set(current);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

function selectedShotId(selection: WorkspaceSelection | undefined): string | undefined {
  return selection?.type === "shot" ? selection.shotId : undefined;
}

export function ProjectStructureTree({
  project,
  tree,
  shots,
  selectedSelection,
  onSelectSelection,
  onCreate,
  openManagement,
  headerActions,
  renderNodeActions,
  shotFilter,
  maxVisibleShots = DEFAULT_MAX_VISIBLE_SHOTS,
}: ProjectStructureTreeProps) {
  const projectId = project.id;
  const series = useMemo(() => orderedSeries(tree), [tree]);
  const shotById = useMemo(() => new Map(shots.map((shot) => [shot.id, shot])), [shots]);
  const sceneByShotId = useMemo(() => {
    const result = new Map<string, string>();
    for (const item of series) {
      for (const episode of orderedEpisodes(item)) {
        for (const scene of orderedScenes(episode)) {
          for (const shotId of scene.shotIds) result.set(shotId, scene.id);
        }
      }
    }
    return result;
  }, [series]);
  const currentSceneId = useMemo(() => {
    if (selectedSelection?.type === "scene") return selectedSelection.sceneId;
    if (selectedSelection?.type === "shot") return sceneByShotId.get(selectedSelection.shotId);
    return undefined;
  }, [sceneByShotId, selectedSelection]);
  const currentShotId = selectedShotId(selectedSelection);
  const [projectExpanded, setProjectExpanded] = useState(true);
  const [expandedSeriesIds, setExpandedSeriesIds] = useState<Set<string>>(
    () => new Set(series.map((item) => item.id)),
  );
  const [expandedEpisodeIds, setExpandedEpisodeIds] = useState<Set<string>>(
    () => new Set(series.flatMap((item) => orderedEpisodes(item).map((episode) => episode.id))),
  );
  const [focusedKey, setFocusedKey] = useState(() => nodeKey("project", projectId));
  const [createMenuOpen, setCreateMenuOpen] = useState(false);
  const nodeRefs = useRef(new Map<string, HTMLDivElement>());

  useEffect(() => {
    setExpandedSeriesIds((current) => {
      const next = new Set<string>();
      for (const item of series) if (current.has(item.id) || !current.size) next.add(item.id);
      return next.size === current.size && [...next].every((id) => current.has(id)) ? current : next;
    });
    const episodeIds = series.flatMap((item) => orderedEpisodes(item).map((episode) => episode.id));
    setExpandedEpisodeIds((current) => {
      const next = new Set<string>();
      for (const id of episodeIds) if (current.has(id) || !current.size) next.add(id);
      return next.size === current.size && [...next].every((id) => current.has(id)) ? current : next;
    });
  }, [series]);

  const visibleShotIdsByScene = useMemo(() => {
    const result = new Map<string, string[]>();
    if (!currentSceneId) return result;
    for (const item of series) {
      for (const episode of orderedEpisodes(item)) {
        const scene = orderedScenes(episode).find((candidate) => candidate.id === currentSceneId);
        if (scene) {
          const matchingShotIds = scene.shotIds.filter((shotId) => {
            const shot = shotById.get(shotId);
            return shot ? (shotFilter?.(shot) ?? true) : true;
          });
          result.set(scene.id, limitVisibleShotIds(matchingShotIds, currentShotId, maxVisibleShots));
        }
      }
    }
    return result;
  }, [currentSceneId, currentShotId, maxVisibleShots, series, shotById, shotFilter]);

  const navigation = useMemo<NavigationModel>(() => {
    const keys: string[] = [];
    const parentByKey = new Map<string, string | undefined>();
    const projectKey = nodeKey("project", projectId);
    const add = (key: string, parent: string | undefined) => {
      keys.push(key);
      parentByKey.set(key, parent);
    };
    add(projectKey, undefined);
    if (!projectExpanded) return { keys, parentByKey };

    for (const item of series) {
      const seriesKey = nodeKey("series", item.id);
      add(seriesKey, projectKey);
      if (!expandedSeriesIds.has(item.id)) continue;
      for (const episode of orderedEpisodes(item)) {
        const episodeKey = nodeKey("episode", episode.id);
        add(episodeKey, seriesKey);
        if (!expandedEpisodeIds.has(episode.id)) continue;
        for (const scene of orderedScenes(episode)) {
          const sceneKey = nodeKey("scene", scene.id);
          add(sceneKey, episodeKey);
          if (currentSceneId !== scene.id) continue;
          for (const shotId of visibleShotIdsByScene.get(scene.id) ?? []) add(nodeKey("shot", shotId), sceneKey);
        }
      }
    }
    const visibleUnassignedShotId = currentShotId && tree.unassignedShotIds.includes(currentShotId) ? currentShotId : undefined;
    if (visibleUnassignedShotId) add(nodeKey("shot", visibleUnassignedShotId), projectKey);
    return { keys, parentByKey };
  }, [currentSceneId, currentShotId, expandedEpisodeIds, expandedSeriesIds, projectExpanded, projectId, series, tree, visibleShotIdsByScene]);

  useEffect(() => {
    setFocusedKey((current) => navigation.keys.includes(current) ? current : navigation.keys[0] ?? nodeKey("project", projectId));
  }, [navigation.keys, projectId]);

  const focusNode = useCallback((key: string) => {
    setFocusedKey(key);
    nodeRefs.current.get(key)?.focus();
  }, []);

  const selectNode = useCallback((next: WorkspaceSelection) => {
    const key = workspaceSelectionKey(next);
    setFocusedKey(key);
    onSelectSelection(next);
  }, [onSelectSelection]);

  const createContext = useMemo<WorkspaceSelection>(() => {
    if (selectedSelection?.type !== "shot") return selectedSelection ?? { type: "project", projectId };
    const sceneId = sceneByShotId.get(selectedSelection.shotId);
    return sceneId ? { type: "scene", sceneId } : { type: "project", projectId };
  }, [projectId, sceneByShotId, selectedSelection]);

  const handleTreeKeyDown = useCallback((event: KeyboardEvent<HTMLDivElement>, key: string, item: TreeItemOptions) => {
    const currentIndex = navigation.keys.indexOf(key);
    const moveTo = (index: number) => {
      const nextKey = navigation.keys[index];
      if (!nextKey) return;
      event.preventDefault();
      focusNode(nextKey);
    };
    if (event.key === "ArrowDown") {
      moveTo(Math.min(navigation.keys.length - 1, currentIndex + 1));
      return;
    }
    if (event.key === "ArrowUp") {
      moveTo(Math.max(0, currentIndex - 1));
      return;
    }
    if (event.key === "Home") {
      moveTo(0);
      return;
    }
    if (event.key === "End") {
      moveTo(navigation.keys.length - 1);
      return;
    }
    if (event.key === "ArrowRight" && item.hasChildren) {
      event.preventDefault();
      if (!item.expanded) item.onToggle?.();
      else moveTo(Math.min(navigation.keys.length - 1, currentIndex + 1));
      return;
    }
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      if (item.hasChildren && item.expanded) item.onToggle?.();
      else {
        const parentKey = navigation.parentByKey.get(key);
        if (parentKey) focusNode(parentKey);
      }
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectNode(item.selection);
    }
  }, [focusNode, navigation, selectNode]);

  const renderTreeItem = (item: TreeItemOptions): ReactNode => {
    const isSelected = isSameWorkspaceSelection(selectedSelection, item.selection);
    const isFocused = focusedKey === item.nodeKey;
    const actions = renderNodeActions?.(item.selection);
    return (
      <div
        key={item.nodeKey}
        ref={(element) => {
          if (element) nodeRefs.current.set(item.nodeKey, element);
          else nodeRefs.current.delete(item.nodeKey);
        }}
        className={`project-structure-treeitem project-structure-treeitem-${item.kind}${isSelected ? " is-selected" : ""}`}
        data-node-type={item.kind}
        data-node-id={item.nodeKey}
        data-focused={isFocused ? "true" : undefined}
        role="treeitem"
        tabIndex={isFocused ? 0 : -1}
        aria-level={item.level}
        aria-posinset={item.position}
        aria-setsize={item.setSize}
        aria-selected={isSelected}
        aria-current={isSelected ? "true" : undefined}
        aria-expanded={item.hasChildren ? item.expanded : undefined}
        onFocus={() => setFocusedKey(item.nodeKey)}
        onKeyDown={(event) => handleTreeKeyDown(event, item.nodeKey, item)}
      >
        <div className="project-structure-tree-row" style={{ paddingLeft: `${8 + Math.max(0, item.level - 1) * 18}px` }}>
          {item.hasChildren ? (
            <button
              type="button"
              className="project-structure-tree-caret"
              tabIndex={-1}
              aria-label={item.expanded ? `折叠${item.label}` : `展开${item.label}`}
              aria-expanded={item.expanded}
              onClick={() => item.onToggle?.()}
            >
              {item.expanded ? "▾" : "▸"}
            </button>
          ) : <span className="project-structure-tree-caret project-structure-tree-caret-placeholder" aria-hidden="true">•</span>}
          <button type="button" className="project-structure-tree-label" onClick={() => selectNode(item.selection)}>
            <span className="project-structure-tree-label-main">{item.label}</span>
            {item.meta && <span className="project-structure-tree-label-meta">{item.meta}</span>}
          </button>
          {actions && <span className="project-structure-tree-actions" onClick={(event) => event.stopPropagation()}>{actions}</span>}
        </div>
        {item.hasChildren && item.expanded && <div className="project-structure-tree-children" role="group">{item.children}</div>}
      </div>
    );
  };

  const renderShot = (shotId: string, position: number, setSize: number): ReactNode => {
    const shot = shotById.get(shotId);
    return renderTreeItem({
      nodeKey: nodeKey("shot", shotId),
      selection: selectionForNode("shot", shotId),
      kind: "shot",
      label: shot?.name ?? `镜头 ${shotId}`,
      meta: shot ? `镜头 ${String(shot.ordinal + 1).padStart(2, "0")}` : `场景内第 ${position} 个`,
      level: 5,
      position,
      setSize,
      hasChildren: false,
    });
  };

  const renderScene = (scene: ProductionScene, position: number, setSize: number): ReactNode => {
    const visibleShotIds = visibleShotIdsByScene.get(scene.id) ?? [];
    const matchingShotCount = scene.shotIds.filter((shotId) => {
      const shot = shotById.get(shotId);
      return shot ? (shotFilter?.(shot) ?? true) : true;
    }).length;
    const isExpanded = currentSceneId === scene.id && matchingShotCount > 0;
    const shotRows = visibleShotIds.map((shotId, index) => renderShot(shotId, index + 1, matchingShotCount));
    const hiddenShotCount = Math.max(0, matchingShotCount - visibleShotIds.length);
    return renderTreeItem({
      nodeKey: nodeKey("scene", scene.id),
      selection: selectionForNode("scene", scene.id),
      kind: "scene",
      label: `Scene ${String(scene.ordinal + 1).padStart(2, "0")} · ${scene.name}`,
      meta: `${matchingShotCount} 个镜头`,
      level: 4,
      position,
      setSize,
      hasChildren: matchingShotCount > 0,
      expanded: isExpanded,
      children: <>
        {shotRows}
        {hiddenShotCount > 0 && <div className="project-structure-tree-limit-note" role="note">还有 {hiddenShotCount} 个镜头未展开，请使用镜头列表搜索/筛选。</div>}
      </>,
    });
  };

  const renderEpisode = (episode: ProductionEpisode, position: number, setSize: number): ReactNode => {
    const scenes = orderedScenes(episode);
    return renderTreeItem({
      nodeKey: nodeKey("episode", episode.id),
      selection: selectionForNode("episode", episode.id),
      kind: "episode",
      label: `第 ${String(episode.ordinal + 1).padStart(2, "0")} 集 · ${episode.name}`,
      meta: `${scenes.length} 个场景`,
      level: 3,
      position,
      setSize,
      hasChildren: scenes.length > 0,
      expanded: expandedEpisodeIds.has(episode.id),
      onToggle: () => setExpandedEpisodeIds((current) => toggleId(current, episode.id)),
      children: scenes.length > 0
        ? scenes.map((scene, index) => renderScene(scene, index + 1, scenes.length))
        : <div className="project-structure-tree-empty">暂无场景</div>,
    });
  };

  const renderSeries = (item: ProductionSeries, position: number, setSize: number): ReactNode => {
    const episodes = orderedEpisodes(item);
    return renderTreeItem({
      nodeKey: nodeKey("series", item.id),
      selection: selectionForNode("series", item.id),
      kind: "series",
      label: `系列 ${String(item.ordinal + 1).padStart(2, "0")} · ${item.name}`,
      meta: `${episodes.length} 集`,
      level: 2,
      position,
      setSize,
      hasChildren: episodes.length > 0,
      expanded: expandedSeriesIds.has(item.id),
      onToggle: () => setExpandedSeriesIds((current) => toggleId(current, item.id)),
      children: episodes.length > 0
        ? episodes.map((episode, index) => renderEpisode(episode, index + 1, episodes.length))
        : <div className="project-structure-tree-empty">暂无集</div>,
    });
  };

  const renderCreateMenuItem = (target: ProjectStructureCreateTarget, label: string): ReactNode => (
    <button
      key={target}
      type="button"
      role="menuitem"
      disabled={!onCreate}
      onClick={() => {
        onCreate?.(target, createContext);
        setCreateMenuOpen(false);
      }}
    >
      {label}
    </button>
  );

  const unassignedSelected = currentShotId && tree.unassignedShotIds.includes(currentShotId) ? currentShotId : undefined;
  const projectChildren = (
    <>
      {series.map((item, index) => renderSeries(item, index + 1, series.length))}
      {tree.unassignedShotIds.length > 0 && (
        <div className="project-structure-tree-unassigned" role="group" aria-label={`未归档镜头，共 ${tree.unassignedShotIds.length} 个`}>
          <div className="project-structure-tree-unassigned-heading">未归档镜头 <span>{tree.unassignedShotIds.length}</span></div>
          {unassignedSelected && renderShot(unassignedSelected, 1, tree.unassignedShotIds.length)}
          {!unassignedSelected && <div className="project-structure-tree-empty">选择未归档镜头后在此定位</div>}
        </div>
      )}
      {!series.length && !tree.unassignedShotIds.length && <div className="project-structure-tree-empty">暂无系列，请使用右上角 + 新建。</div>}
    </>
  );

  return (
    <section className="project-structure-tree-panel" aria-labelledby="project-structure-tree-title">
      <header className="project-structure-tree-header">
        <div>
          <span className="project-structure-tree-eyebrow">Project structure</span>
          <h2 id="project-structure-tree-title">项目结构</h2>
          <p>选择 Series、Episode、Scene 或 Shot，主工作区会切换对应上下文。</p>
        </div>
        <div className="project-structure-tree-header-actions">
          {headerActions}
          {openManagement && <button type="button" className="project-structure-tree-manage-button" onClick={() => openManagement(createContext)}>管理</button>}
          <div className="project-structure-tree-create">
            <button
              type="button"
              className="project-structure-tree-create-button"
              aria-label="新建"
              aria-haspopup="menu"
              aria-expanded={createMenuOpen}
              onClick={() => setCreateMenuOpen((open) => !open)}
            >
              <span aria-hidden="true">＋</span> 新建
            </button>
            {createMenuOpen && <div className="project-structure-tree-create-menu" role="menu" aria-label="新建菜单">
              {renderCreateMenuItem("shot", "新建 Shot")}
              {renderCreateMenuItem("series", "新建 Series")}
              {renderCreateMenuItem("episode", "新建 Episode")}
              {renderCreateMenuItem("scene", "新建 Scene")}
            </div>}
          </div>
        </div>
      </header>
      <div className="project-structure-tree-summary">默认展开 Series / Episode；仅展开当前 Scene 的 Shot，共 {shots.length} 个 Shot。</div>
      <div className="project-structure-tree" role="tree" aria-label={`${project.name}项目结构`}>
        {renderTreeItem({
          nodeKey: nodeKey("project", projectId),
          selection: selectionForNode("project", projectId),
          kind: "project",
          label: project.name,
          meta: `${series.length} 个系列`,
          level: 1,
          position: 1,
          setSize: 1,
          hasChildren: true,
          expanded: projectExpanded,
          onToggle: () => setProjectExpanded((expanded) => !expanded),
          children: projectChildren,
        })}
      </div>
    </section>
  );
}
