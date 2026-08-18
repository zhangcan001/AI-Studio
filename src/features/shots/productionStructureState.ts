import type {
  ProductionEpisode,
  ProductionScene,
  ProductionSeries,
  ProductionStructureTree,
} from "../../types/productionStructure";

export interface ProductionSceneOption {
  value: string;
  label: string;
}

export const EMPTY_PRODUCTION_STRUCTURE = (projectId: string): ProductionStructureTree => ({
  projectId,
  series: [],
  unassignedShotIds: [],
});

export function orderedSeries(tree: ProductionStructureTree): ProductionSeries[] {
  return [...tree.series].sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
}

export function orderedEpisodes(series: ProductionSeries): ProductionEpisode[] {
  return [...series.episodes].sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
}

export function orderedScenes(episode: ProductionEpisode): ProductionScene[] {
  return [...episode.scenes].sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
}

export function productionSceneOptions(tree: ProductionStructureTree): ProductionSceneOption[] {
  return [
    { value: "ALL", label: "全部镜头" },
    { value: "UNASSIGNED", label: `未归档（${tree.unassignedShotIds.length}）` },
    ...orderedSeries(tree).flatMap((series) => orderedEpisodes(series).flatMap((episode) => orderedScenes(episode).map((scene) => ({
      value: scene.id,
      label: `S${String(series.ordinal + 1).padStart(2, "0")} / E${String(episode.ordinal + 1).padStart(2, "0")} / ${scene.name}`,
    })))),
  ];
}

export function shotSceneIndex(tree: ProductionStructureTree): Record<string, string> {
  const result: Record<string, string> = {};
  for (const series of orderedSeries(tree)) {
    for (const episode of orderedEpisodes(series)) {
      for (const scene of orderedScenes(episode)) {
        for (const shotId of scene.shotIds) result[shotId] = scene.id;
      }
    }
  }
  return result;
}

export function findProductionScene(tree: ProductionStructureTree, sceneId?: string): ProductionScene | undefined {
  if (!sceneId) return undefined;
  for (const series of tree.series) {
    for (const episode of series.episodes) {
      const scene = episode.scenes.find((item) => item.id === sceneId);
      if (scene) return scene;
    }
  }
  return undefined;
}

export function findProductionSceneParent(
  tree: ProductionStructureTree,
  sceneId: string,
): { series: ProductionSeries; episode: ProductionEpisode; scene: ProductionScene } | undefined {
  for (const series of tree.series) {
    for (const episode of series.episodes) {
      const scene = episode.scenes.find((item) => item.id === sceneId);
      if (scene) return { series, episode, scene };
    }
  }
  return undefined;
}

export function moveOrderedId(ids: string[], index: number, delta: -1 | 1): string[] {
  const nextIndex = index + delta;
  if (index < 0 || index >= ids.length || nextIndex < 0 || nextIndex >= ids.length) return ids;
  const next = [...ids];
  [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
  return next;
}

export function normalizeStructureName(value: string): string | undefined {
  const name = value.trim().replace(/[\r\n]+/g, " ");
  return name ? name.slice(0, 100) : undefined;
}
