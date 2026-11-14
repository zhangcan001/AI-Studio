import { useEffect, useRef, useState } from "react";
import { productClient } from "../product/client";
import { normalizeProductError } from "../product/errors";
/** Resolve by project and Asset identity; never expose an arbitrary caller path. */
export function ResultFileActions({projectId,assetId}: {projectId:string;assetId:string}) {
  return <ScopedResultFileActions key={JSON.stringify([projectId,assetId])} projectId={projectId} assetId={assetId}/>;
}
function ScopedResultFileActions({projectId,assetId}:{projectId:string;assetId:string}) {
  const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [busy,setBusy]=useState(false),[error,setError]=useState<string>();
  const lock=useRef(false),owner=useRef(""); const scope=JSON.stringify([projectId,assetId]);owner.current=scope;
  const reveal=async()=>{if(lock.current)return;lock.current=true;setBusy(true);setError(undefined);try{await productClient.library.reveal(projectId,assetId);}catch(cause){if(mounted.current&&owner.current===scope)setError(normalizeProductError(cause).message);}finally{lock.current=false;if(mounted.current&&owner.current===scope)setBusy(false);}};
  return <span className="result-file-actions"><button type="button" disabled={busy} onClick={()=>void reveal()}>{busy?"正在定位…":"在文件夹中显示"}</button>{error&&<small role="alert">{error}</small>}</span>;
}
