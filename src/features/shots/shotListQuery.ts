import type { ShotView } from "../../types/shot";
import type { ShotStatus } from "./shotDomain";
import { deriveShotStatus } from "./shotDomain";

export type ShotListStatusFilter = "ALL" | ShotStatus;

export const SHOT_LIST_PAGE_SIZES = [25, 50, 100] as const;
export const DEFAULT_SHOT_LIST_PAGE_SIZE = 50;

export const SHOT_LIST_STATUS_OPTIONS: ReadonlyArray<{
  value: ShotListStatusFilter;
  label: string;
}> = [
  { value: "ALL", label: "全部" },
  { value: "DRAFT", label: "待配置" },
  { value: "READY", label: "待生成" },
  { value: "GENERATING_IMAGE", label: "图片处理中" },
  { value: "IMAGE_REVIEW", label: "图片待审核" },
  { value: "IMAGE_SELECTED", label: "图片已选" },
  { value: "GENERATING_VIDEO", label: "视频处理中" },
  { value: "VIDEO_REVIEW", label: "视频待审核" },
  { value: "COMPLETED", label: "已完成" },
  { value: "FAILED", label: "失败" },
];

export interface ShotListControls {
  query: string;
  status: ShotListStatusFilter;
  sceneId: string;
  pageSize: number;
  page: number;
}

export interface ShotListView {
  filteredShots: ShotView[];
  pageShots: ShotView[];
  page: number;
  pageCount: number;
  filteredCount: number;
  pageStart: number;
  pageEnd: number;
  isFiltered: boolean;
}

export function defaultShotListControls(): ShotListControls {
  return { query: "", status: "ALL", sceneId: "ALL", pageSize: DEFAULT_SHOT_LIST_PAGE_SIZE, page: 1 };
}

export function updateShotListControls(
  controls: ShotListControls,
  change: Partial<Pick<ShotListControls, "query" | "status" | "sceneId" | "pageSize">>,
): ShotListControls {
  return { ...controls, ...change, page: 1 };
}

export function isShotListFiltered(controls: Pick<ShotListControls, "query" | "status" | "sceneId" | "pageSize">): boolean {
  return Boolean(controls.query.trim()) || controls.status !== "ALL" || controls.sceneId !== "ALL";
}

export function isShotListReorderDisabled(controls: Pick<ShotListControls, "query" | "status" | "sceneId" | "pageSize">): boolean {
  return isShotListFiltered(controls);
}

export function buildShotListView(shots: ShotView[], controls: ShotListControls, shotSceneIds: Readonly<Record<string, string>> = {}): ShotListView {
  const query = controls.query.trim().toLocaleLowerCase();
  const filteredShots = shots
    .filter((shot) => {
      if (query && !`${shot.name}\n${shot.promptText}`.toLocaleLowerCase().includes(query)) return false;
      if (controls.status !== "ALL" && deriveShotStatus(shot) !== controls.status) return false;
      if (controls.sceneId === "UNASSIGNED") return !shotSceneIds[shot.id];
      return controls.sceneId === "ALL" || shotSceneIds[shot.id] === controls.sceneId;
    })
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
  const pageCount = Math.max(1, Math.ceil(filteredShots.length / controls.pageSize));
  const page = Math.min(Math.max(1, controls.page), pageCount);
  const pageStart = filteredShots.length ? (page - 1) * controls.pageSize + 1 : 0;
  const pageEnd = Math.min(page * controls.pageSize, filteredShots.length);

  return {
    filteredShots,
    pageShots: filteredShots.slice(pageStart ? pageStart - 1 : 0, pageEnd),
    page,
    pageCount,
    filteredCount: filteredShots.length,
    pageStart,
    pageEnd,
    isFiltered: isShotListReorderDisabled(controls),
  };
}
