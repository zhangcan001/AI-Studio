import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import type { ShotView } from "../../types/shot";
import { normalizeProductError } from "../../product/errors";
import { ShotBulkConfigPanel } from "./ShotBulkConfigPanel";
export function StoryboardBatchDialog({projectId,onClose,onQueue,onShot}: {projectId:string;onClose:()=>void;onQueue:(id?:string)=>void;onShot:(id:string)=>void}) {
  const dialog=useRef<HTMLDialogElement>(null);
  const [busy,setBusy]=useState(false);
  const [shots,setShots]=useState<ShotView[]>(),[error,setError]=useState<string>();
  const mounted=useRef(true),loading=useRef(0);
  const reload=async()=>{const token=++loading.current;try{const next=await productClient.creation.shotsList(projectId);if(mounted.current&&token===loading.current){setShots(next.filter(s=>s.projectId===projectId));setError(undefined);}}catch(cause){if(mounted.current&&token===loading.current)setError(normalizeProductError(cause).message);}};
  useEffect(()=>{mounted.current=true;const opener=document.activeElement;dialog.current?.showModal();void reload();return()=>{mounted.current=false;++loading.current;if(opener instanceof HTMLElement&&opener.isConnected)opener.focus();};},[projectId]);
  return <dialog ref={dialog} className="storyboard-batch-dialog" aria-label="批量准备视频" onCancel={e=>{if(busy)e.preventDefault();else onClose();}}>
    <header><h2>批量准备视频</h2><button type="button" disabled={busy} onClick={onClose}>返回创作</button></header>
    <p>1 选择镜头 → 2 正式预检 → 3 准备冻结批次 → 4 在队列明确启动。准备不会运行 GPU；修改镜头不会改写旧批次。</p>
    {error&&<p role="alert">{error}</p>}{shots?<ShotBulkConfigPanel projectId={projectId} shots={shots} videoOnly confirmPromptOverwrite onBulkPrompt={async(stage,ids,text)=>{if(stage!=="video")throw new Error("仅支持视频提示词");await productClient.creation.shotsPromptAssign(projectId,ids,text);}} onBusyChange={setBusy} onRefresh={reload} onOpenProductionQueue={onQueue} onOpenShot={onShot}/>:<p role="status">正在读取镜头…</p>}
  </dialog>;
}
