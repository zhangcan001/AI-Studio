import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import { orderedEpisodes, orderedScenes, orderedSeries } from "./productionStructureState";

import type { VideoPreparationScope } from "../../types/productionPreparation";
export type { VideoPreparationScope } from "../../types/productionPreparation";

// A selection projection only. The backend remains the readiness/admission and
// frozen-input authority; Scene assignment order wins over global Shot ordinal.
export function videoPreparationShots(projectId: string, shots: ShotView[], tree?: ProductionStructureTree, scope: VideoPreparationScope = { kind: "project" }): ShotView[] {
  const owned = shots.filter(shot => shot.projectId === projectId);
  const fallback = [...owned].sort((a, b) => a.ordinal - b.ordinal || a.id.localeCompare(b.id));
  if (!tree) {
    if (scope.kind !== "project") throw Error("生产范围缺少正式项目结构。");
    return fallback;
  }
  if (tree.projectId !== projectId) throw Error("生产结构不属于当前项目。");
  const byId = new Map(owned.map(shot => [shot.id, shot]));
  const assigned = new Set<string>(), selected = new Set<string>();
  for (const series of orderedSeries(tree)) {
    if (series.projectId !== projectId) throw Error("系列归属不一致。");
    for (const episode of orderedEpisodes(series)) {
      if (episode.seriesId !== series.id) throw Error("集归属不一致。");
      for (const scene of orderedScenes(episode)) {
        if (scene.episodeId !== episode.id) throw Error("场景归属不一致。");
        const include = scope.kind === "project" || scope.kind === "series" && scope.id === series.id
          || scope.kind === "episode" && scope.id === episode.id || scope.kind === "scene" && scope.id === scene.id;
        for (const id of scene.shotIds) {
          if (!byId.has(id)) throw Error("结构包含未知或其他项目的镜头，请刷新结构。");
          if (assigned.has(id)) throw Error("镜头存在重复场景归属，请刷新结构。");
          assigned.add(id);
          if (include) selected.add(id);
        }
      }
    }
  }
  if (scope.kind === "project" || scope.kind === "unassigned") {
    for (const shot of fallback) if (!assigned.has(shot.id)) selected.add(shot.id);
  }
  return [...selected].map(id => byId.get(id)!);
}
