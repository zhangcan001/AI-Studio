import { useLibraryMediaInspection } from "./useLibraryMediaInspection";
import { useCallback, useEffect, useRef, useState } from "react";
import type { AppRoute } from "../../app/routes/types";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns, subscribeRunInvalidation } from "../../product/runInvalidation";
import type { LibraryDeletionInspection, LibraryDetail, LibraryEditRequest, LibraryList, LibraryRelation, LibraryVersions, LibraryTag, ResourceRef } from "../../product/libraryTypes";
import { useStudioStore } from "../../stores/studioStore";
import { categoryFor, resourceKey, type LibraryRoute } from "./libraryModel";
export interface LibraryProps { route: LibraryRoute; navigate: (route: AppRoute) => unknown }
interface Projection {scope:string;list:LibraryList;detail?:LibraryDetail;relations:LibraryRelation[];versions?:LibraryVersions}
export function useLibraryController({route,navigate}:LibraryProps) {
  const [queryState,setQueryState]=useState({projectId:route.projectId,search:"",favoriteOnly:false,tagId:""});
  const [tagProjection,setTagProjection]=useState<{projectId:string;items:LibraryTag[]}>();
  const mediaCategory=["media","images","videos","audio"].includes(categoryFor(route));
  const ownedQuery=queryState.projectId===route.projectId ? queryState : {projectId:route.projectId,search:"",favoriteOnly:false,tagId:""};
  const search=ownedQuery.search, favoriteOnly=mediaCategory&&ownedQuery.favoriteOnly, tagId=mediaCategory ? ownedQuery.tagId.trim() : "";
  const setSearch=(search:string)=>setQueryState(state=>({...state,projectId:route.projectId,search}));
  const setFavoriteOnly=(favoriteOnly:boolean)=>setQueryState(state=>({...state,projectId:route.projectId,favoriteOnly}));
  const setTagId=(tagId:string)=>setQueryState(state=>({...state,projectId:route.projectId,tagId}));
  const mediaInspection=useLibraryMediaInspection(route.projectId,route.resource);
  const [projection,setProjection]=useState<Projection>();
  const [queryError,setQueryError]=useState<string>();const [actionError,setActionError]=useState<string>();const [notice,setNotice]=useState<string>();
  const [inspection,setInspection]=useState<LibraryDeletionInspection>();const [busy,setBusy]=useState(false);const [loading,setLoading]=useState(true);
  const inspectionOpen=useRef(false);const epoch=useRef(0);const actionLock=useRef(false);
  const [pageIndex,setPageIndex]=useState(0);
  const pages=useRef<{owner:string;index:number;starts:LibraryList["nextCursor"][]}>({owner:"",index:0,starts:[null]});
  const tagsOwner=useRef<string|undefined>(undefined);
  const navigating=useRef(false);
  const selfEditInvalidation=useRef(false);
  const category=categoryFor(route);const queryScope=JSON.stringify([route.projectId,category,search.trim(),favoriteOnly,tagId]);
  const scope=JSON.stringify([queryScope,resourceKey(route.resource)]);
  const liveScope=useRef(scope);liveScope.current=scope;
  const previousProject=useRef(route.projectId);
  const refresh=useCallback(async(options?:{tags?:boolean;page?:number;cursor?:LibraryList["nextCursor"]})=>{
    const token=++epoch.current;
    navigating.current=options?.page!==undefined;
    const target=options?.page??pages.current.index;
    const cursor=options?.cursor!==undefined?options.cursor:pages.current.starts[target]??null;
    const owns=()=>token===epoch.current&&liveScope.current===scope;
    setLoading(true);
    try {
      if(mediaCategory&&(options?.tags!==false||tagsOwner.current!==route.projectId)) {
        const tags=await productClient.library.tagsList(route.projectId);
        if(!owns())return;
        const ownedTags=tags.filter(tag=>tag.projectId===route.projectId);
        tagsOwner.current=route.projectId;
        setTagProjection({projectId:route.projectId,items:ownedTags});
        if(tagId&&!ownedTags.some(tag=>tag.id===tagId)) {
          setProjection(undefined);
          setQueryState(state=>({...state,tagId:""}));return;
        }
      }
      const list=await productClient.library.list(route.projectId,{category,keyword:search.trim()||null,favoriteOnly,tagId:tagId||null,cursor,limit:30});
      if(!owns())return;
      let detail:LibraryDetail|undefined;let relations:LibraryRelation[]=[];let versions:LibraryVersions|undefined;
      if(route.resource) {
        try {
          [detail,relations,versions]=await Promise.all([productClient.library.get(route.projectId,route.resource),productClient.library.relationsGet(route.projectId,route.resource),productClient.library.versionsGet(route.projectId,route.resource)]);
        }catch(error) {
          if(!owns())return;const e=normalizeProductError(error);
          if(e.code!=="LIBRARY_RESOURCE_NOT_FOUND"&&e.code!=="PROJECT_SCOPE_VIOLATION")throw e;
          setNotice("资源不存在或不可访问，已返回资源列表。");navigate({...route,resource:undefined});
        }
      }
      if(route.resource&&inspectionOpen.current){const nextInspection=await productClient.library.deletionInspect(route.projectId,route.resource);if(owns()&&inspectionOpen.current)setInspection(nextInspection);}
      if(owns()){
        // Commit cursor history only for the latest successful request. Old pages
        // are not cached and are never replayed on refresh or navigation.
        const starts=pages.current.starts.slice(0,target+1);starts[target]=cursor;
        pages.current={owner:queryScope,index:target,starts};setPageIndex(target);
        setProjection({scope,list,detail,relations,versions});setQueryError(undefined);
      }
    }catch(error){if(owns())setQueryError(normalizeProductError(error).message);}
    finally{if(owns()){navigating.current=false;setLoading(false);}}
  },[scope]);
  useEffect(()=>{
    inspectionOpen.current=false;setProjection(undefined);setInspection(undefined);setQueryError(undefined);setActionError(undefined);setLoading(true);
    if(previousProject.current!==route.projectId){previousProject.current=route.projectId;setQueryState({projectId:route.projectId,search:"",favoriteOnly:false,tagId:""});setTagProjection(undefined);setNotice(undefined);
      if(route.resource)navigate({...route,resource:undefined});}
    if(pages.current.owner!==queryScope){pages.current={owner:queryScope,index:0,starts:[null]};setPageIndex(0);}
    void refresh({tags:false});
    let event:ReturnType<typeof setTimeout>|undefined;
    const unsubscribe=subscribeRunInvalidation(project=>{if(project!==route.projectId)return;if(selfEditInvalidation.current){selfEditInvalidation.current=false;return;}clearTimeout(event);event=setTimeout(()=>void refresh(),150);});
    const timer=setInterval(()=>{if(!navigating.current)void refresh({tags:false});},5000);
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
  return {mediaInspection,returnRoute,category,search,setSearch,mediaCategory,favoriteOnly,setFavoriteOnly,tagId,setTagId,tags:tagProjection?.projectId===route.projectId?tagProjection.items:[],list:current?.list,detail:current?.detail,relations:current?.relations??[],versions:current?.versions,queryError,actionError,notice,inspection,busy,loading,refresh,inspectDelete,confirmDelete,edit,useInCreation,
    pageIndex,canPrevious:pageIndex>0,
    nextPage:()=>{if(current?.list.nextCursor)void refresh({tags:false,page:pages.current.index+1,cursor:current.list.nextCursor});},
    previousPage:()=>{if(pages.current.index>0)void refresh({tags:false,page:pages.current.index-1});},
    open:(resource:ResourceRef)=>{setNotice(undefined);navigate({...route,resource});},
    closeInspection:()=>{inspectionOpen.current=false;setInspection(undefined);},
  };
}
export type LibraryController=ReturnType<typeof useLibraryController>;
