import { useCallback, useEffect, useRef, useState } from "react";
import type { AppRoute } from "../../app/routes/types";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns, subscribeRunInvalidation } from "../../product/runInvalidation";
import type { LibraryDeletionInspection, LibraryDetail, LibraryEditRequest, LibraryList, LibraryRelation, LibraryVersions, ResourceRef } from "../../product/libraryTypes";
import { useStudioStore } from "../../stores/studioStore";
import { categoryFor, resourceKey, type LibraryRoute } from "./libraryModel";
export interface LibraryProps { route: LibraryRoute; navigate: (route: AppRoute) => unknown }
interface Projection {scope:string;list:LibraryList;detail?:LibraryDetail;relations:LibraryRelation[];versions?:LibraryVersions}
export function useLibraryController({route,navigate}:LibraryProps) {
  const [search,setSearch]=useState("");const [projection,setProjection]=useState<Projection>();
  const [queryError,setQueryError]=useState<string>();const [actionError,setActionError]=useState<string>();const [notice,setNotice]=useState<string>();
  const [inspection,setInspection]=useState<LibraryDeletionInspection>();const [busy,setBusy]=useState(false);const [loading,setLoading]=useState(true);
  const inspectionOpen=useRef(false);const epoch=useRef(0);const actionLock=useRef(false);const pageCount=useRef(1);
  const selfEditInvalidation=useRef(false);
  const category=categoryFor(route);const scope=`${route.projectId}:${category}:${search.trim()}:${resourceKey(route.resource)}`;
  const liveScope=useRef(scope);liveScope.current=scope;
  const previousProject=useRef(route.projectId);
  const refresh=useCallback(async()=>{
    const token=++epoch.current;
    try {
      let list:LibraryList|undefined;const items:LibraryList["items"]=[];let cursor:LibraryList["nextCursor"]=null;
      for(let index=0;index<pageCount.current;index++) {
        const page=await productClient.library.list(route.projectId,{category,keyword:search.trim()||null,cursor,limit:30});
        if(token!==epoch.current)return;
        items.push(...page.items);list=page;cursor=page.nextCursor;if(!cursor)break;
      }
      if(!list)return;list={...list,items};
      let detail:LibraryDetail|undefined;let relations:LibraryRelation[]=[];let versions:LibraryVersions|undefined;
      if(route.resource) {
        try {
          [detail,relations,versions]=await Promise.all([productClient.library.get(route.projectId,route.resource),productClient.library.relationsGet(route.projectId,route.resource),productClient.library.versionsGet(route.projectId,route.resource)]);
        }catch(error) {
          if(token!==epoch.current)return;const e=normalizeProductError(error);
          if(e.code!=="LIBRARY_RESOURCE_NOT_FOUND"&&e.code!=="PROJECT_SCOPE_VIOLATION")throw e;
          setNotice("资源不存在或不可访问，已返回资源列表。");navigate({...route,resource:undefined});
        }
      }
      if(route.resource&&inspectionOpen.current){const nextInspection=await productClient.library.deletionInspect(route.projectId,route.resource);if(token===epoch.current&&inspectionOpen.current)setInspection(nextInspection);}
      if(token===epoch.current){setProjection({scope,list,detail,relations,versions});setQueryError(undefined);}
    }catch(error){if(token===epoch.current)setQueryError(normalizeProductError(error).message);}
    finally{if(token===epoch.current)setLoading(false);}
  },[scope]);
  useEffect(()=>{
    inspectionOpen.current=false;setProjection(undefined);setInspection(undefined);setQueryError(undefined);setActionError(undefined);setLoading(true);
    if(previousProject.current!==route.projectId){previousProject.current=route.projectId;setSearch("");setNotice(undefined);}
    pageCount.current=1;void refresh();
    let event:ReturnType<typeof setTimeout>|undefined;
    const unsubscribe=subscribeRunInvalidation(project=>{if(project!==route.projectId)return;if(selfEditInvalidation.current){selfEditInvalidation.current=false;return;}clearTimeout(event);event=setTimeout(()=>void refresh(),150);});
    const timer=setInterval(()=>void refresh(),5000);
    return()=>{++epoch.current;unsubscribe();clearTimeout(event);clearInterval(timer);};
  },[refresh]);
  const current=projection?.scope===scope?projection:undefined;
  async function mutate(action:()=>Promise<void>){if(actionLock.current)return false;actionLock.current=true;setBusy(true);setActionError(undefined);const owner=scope;
    try{await action();return liveScope.current===owner;}catch(error){if(liveScope.current===owner)setActionError(normalizeProductError(error).message);return false;}
    finally{actionLock.current=false;setBusy(false);}
  }
  const inspectDelete=()=>mutate(async()=>{if(!route.resource)return;const result=await productClient.library.deletionInspect(route.projectId,route.resource);if(liveScope.current===scope){inspectionOpen.current=true;setInspection(result);}});
  const confirmDelete=()=>mutate(async()=>{if(!route.resource||!inspection?.allowed)return;await productClient.library.delete(route.projectId,route.resource,true);invalidateRuns(route.projectId);if(liveScope.current===scope){inspectionOpen.current=false;setInspection(undefined);setNotice("资源已删除；历史任务和运行记录仍保留。");navigate({...route,resource:undefined});}});
  const edit=(request:LibraryEditRequest)=>mutate(async()=>{await productClient.library.resourceEdit(route.projectId,request);
    // Dispatch is synchronous: only this publication is self-originated. Never
    // suppress later external events or the polling fallback.
    selfEditInvalidation.current=true;try{invalidateRuns(route.projectId);}finally{selfEditInvalidation.current=false;}
    if(liveScope.current===scope){inspectionOpen.current=false;setInspection(undefined);await refresh();}});
  const savedReturn=useStudioStore(state=>state.creationLabReturn);
  const returnRoute=savedReturn?.route?.projectId===route.projectId && savedReturn.scope===`${savedReturn.route.projectId}:${savedReturn.route.shotId ?? ""}:${savedReturn.route.stage}` ? savedReturn.route : undefined;
  const useInCreation=(stage?: "image" | "video")=>mutate(async()=>{
    if(!route.resource || (!returnRoute && !stage))return;
    const intent=await productClient.library.useInCreation(route.projectId,route.resource);
    if(liveScope.current!==scope || intent.projectId!==route.projectId || (intent.kind==="asset" && intent.mediaKind==="audio"))return;
    useStudioStore.getState().setPendingLibraryIntent(intent);
    navigate(returnRoute ?? {kind:"create",projectId:route.projectId,stage:stage!});
  });
  return {returnRoute,category,search,setSearch,list:current?.list,detail:current?.detail,relations:current?.relations??[],versions:current?.versions,queryError,actionError,notice,inspection,busy,loading,refresh,inspectDelete,confirmDelete,edit,useInCreation,
    loadMore:()=>{if(current?.list.nextCursor){pageCount.current++;void refresh();}},
    open:(resource:ResourceRef)=>{setNotice(undefined);navigate({...route,resource});},
    closeInspection:()=>{inspectionOpen.current=false;setInspection(undefined);},
  };
}
export type LibraryController=ReturnType<typeof useLibraryController>;
