import type { AppRoute } from "../../app/routes/types";
import type { LibraryCategory, LibraryDetail, LibraryRelationLocation, ResourceRef } from "../../product/libraryTypes";
export type LibraryRoute = Extract<AppRoute, { kind: "library" }>;
export const categories: { key: LibraryCategory; label: string }[] = [{key:"all",label:"近期资源"},{key:"media",label:"媒体"},{key:"images",label:"图片"},{key:"videos",label:"视频"},{key:"audio",label:"音频"},{key:"prompts",label:"提示词"},{key:"profiles",label:"角色 / 场景 / 道具 / 风格"},{key:"reference-sets",label:"参考集"}];
export const categoryFor = (route: LibraryRoute): LibraryCategory => categories.find(c=>c.key===route.filter)?.key ?? "all";
export const resourceKey = (resource?: ResourceRef) => resource ? `${resource.kind}:${resource.id}` : "";
export const normalLibrary = (route: AppRoute) => route.kind==="library" && !["advanced-assets","advanced-prompts"].includes(route.filter ?? "");
export const subtypeLabel = (type: string) => ({image:"图片",video:"视频",audio:"音频",prompt:"提示词",snippet:"片段",CHARACTER:"角色设定",SCENE:"场景设定",PROP:"道具设定",STYLE:"风格设定",SHOT:"镜头参考集",COSTUME:"服装参考集"} as Record<string,string>)[type] ?? "参考资源";
export function profileValue(detail: Extract<LibraryDetail,{kind:"profile"}>) {
  const p=detail.profile;
  return "Character" in p ? p.Character : "Scene" in p ? p.Scene : "Prop" in p ? p.Prop : p.Style;
}
export function detailTitle(detail: LibraryDetail) {return detail.kind==="asset" ? detail.asset.name : detail.kind==="prompt" ? detail.prompt.name : detail.kind==="reference-set" ? detail.referenceSet.name : profileValue(detail).name;}
export function relationRoute(projectId: string, location: LibraryRelationLocation): AppRoute {
  if(location.kind==="run") return {kind:"runs",projectId,run:location.runRef};
  if(location.kind==="shot") return {kind:"create",projectId,shotId:location.id,stage:location.stage==="video"?"video":"image"};
  return {kind:"library",projectId,resource:location.resource};
}

/** Presentation only: never decides whether a protected resource may be deleted. */
export function deletionMessage(message:string) {
  if(message.includes("selected image asset") || message.includes("selected video asset") || message.includes("选为关键帧")) return "该素材当前已被镜头选用。请先更换镜头结果后再删除。";
  if(message.includes("ReferenceSet"))return "该素材仍被参考集使用，请先从参考集移除素材。";
  if(message.includes("Shot Reference"))return "该素材仍被镜头长期参考使用，请先解除镜头引用。";
  if(message.includes("Reference Anchor"))return "该素材仍被参考锚点使用，请先解除对应引用。";
  return message;
}
