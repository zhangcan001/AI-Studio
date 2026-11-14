import { defaultGenerationValues } from "../../stores/studioStore";
import type { GeneratorOption } from "../../product/types";
import type { ShotInputValues } from "../../types/shot";
import { shotDurationMax } from "../../product/shotDuration";
import { useCallback, useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import type { ShotView } from "../../types/shot";
import { normalizeProductError } from "../../product/errors";
import { ShotBulkConfigPanel } from "./ShotBulkConfigPanel";
type Props={projectId:string;onClose:()=>void;onQueue:(id?:string)=>void;onShot:(id:string)=>void};
export function StoryboardBatchDialog(props:Props){return <ScopedStoryboardBatchDialog key={props.projectId} {...props}/>;}
function ScopedStoryboardBatchDialog({projectId,onClose,onQueue,onShot}:Props) {
  const dialog=useRef<HTMLDialogElement>(null);
  const [busy,setBusy]=useState(false);
  const [generators,setGenerators]=useState<GeneratorOption[]>([]),[selection,setSelection]=useState("");
  const [duration,setDuration]=useState(""),[presetId,setPresetId]=useState("");
  const generator=generators.find(g=>g.selectionRef===selection&&g.availability);
  const durationField=generator?.fields.find(f=>f.key==="duration_seconds"&&(f.type==="integer"||f.type==="number"));
  const dirty=useRef(false);const reportDirty=useCallback((next:boolean)=>{dirty.current=next;},[]);
  const leave=(fn:()=>void)=>{if(!busy&&(!(dirty.current||selection)||window.confirm("批量提示词或参数尚未应用，确定放弃并离开？")))fn();};
  const [shots,setShots]=useState<ShotView[]>(),[error,setError]=useState<string>();
  const mounted=useRef(true),loading=useRef(0);
  const reload=async()=>{const token=++loading.current;try{const next=await productClient.creation.shotsList(projectId);if(mounted.current&&token===loading.current){setShots(next.filter(s=>s.projectId===projectId));setError(undefined);}}catch(cause){if(mounted.current&&token===loading.current)setError(normalizeProductError(cause).message);}};
  useEffect(()=>{mounted.current=true;const opener=document.activeElement;dialog.current?.showModal();void reload();return()=>{mounted.current=false;++loading.current;if(opener instanceof HTMLElement&&opener.isConnected)opener.focus();};},[projectId]);
  useEffect(()=>{let alive=true;void Promise.resolve().then(()=>productClient.creation.generatorsList(projectId,"video")).then(next=>{if(alive)setGenerators(next);}).catch(cause=>{if(alive)setError(normalizeProductError(cause).message);});return()=>{alive=false;};},[projectId]);
  const configure=async(stage:"image"|"video",shotIds:string[])=>{
    if(stage!=="video"||!generator)throw new Error("请选择正式可用的生成器");
    const preset=generator.resolutionPresets?.find(p=>p.id===presetId);
    const values=Object.fromEntries(Object.entries(defaultGenerationValues({fields:generator.fields})).filter(([,v])=>["integer","number","seed_fixed","seed_random"].includes(v.type))) as ShotInputValues;
    if(durationField&&duration!==""){
      const n=Number(duration),max=shotDurationMax("max" in durationField?durationField.max:undefined),min="min" in durationField?durationField.min:undefined;
      if(!Number.isFinite(n)||(min!==undefined&&n<min)||(max!==undefined&&n>max)||(durationField.type==="integer"&&!Number.isInteger(n))){setError("请按生成器范围输入合法时长。");throw new Error("INVALID_INPUT");}
      values.duration_seconds={type:durationField.type==="number"?"number":"integer",value:n};
    }
    if(generator.resolutionPresets?.length&&!preset){setError("请选择分辨率。");throw new Error("INVALID_INPUT");}
    if(preset){values.width={type:"integer",value:preset.width};values.height={type:"integer",value:preset.height};}
    if(!window.confirm(`将覆盖所选 ${shotIds.length} 个镜头的生成器及标量参数；保留各 Recipe 的媒体输入、提示词和旧批次。不会启动生成。是否应用？`))return false;
    await productClient.creation.shotsConfigure(projectId,{shotIds,selectionRef:generator.selectionRef,values});
    setSelection("");setError(undefined);
  };
  const configurationEditor=<fieldset disabled={busy}><legend>批量生成器与参数</legend><p>只修改明确勾选的镜头；媒体仍需按各自正式槽位绑定，不会复制首帧到尾帧。</p>
    <label>批量生成器<select aria-label="批量生成器" value={selection} onChange={e=>{const g=generators.find(g=>g.selectionRef===e.target.value);setSelection(e.target.value);const f=g?.fields.find(f=>f.key==="duration_seconds"&&(f.type==="integer"||f.type==="number"));setDuration(f&&"default" in f&&f.default!==undefined?String(f.default):"");setPresetId(g?.resolutionPresets?.[0]?.id??"");}}><option value="">请选择要应用的生成器</option>{generators.map(g=><option key={g.selectionRef} value={g.selectionRef} disabled={!g.availability}>{g.name} · {g.mode} · {g.version}</option>)}</select></label>
    {durationField&&<label>批量时长（秒）<input aria-label="批量时长（秒）" type="number" min={"min" in durationField?durationField.min:undefined} max={shotDurationMax("max" in durationField?durationField.max:undefined)} step={durationField.type==="integer"?1:"any"} value={duration} onChange={e=>setDuration(e.target.value)}/></label>}
    {generator?.resolutionPresets?.length? <label>批量画幅与分辨率<select aria-label="批量画幅与分辨率" value={presetId} onChange={e=>setPresetId(e.target.value)}>{generator.resolutionPresets.map(p=><option key={p.id} value={p.id}>{p.label} · {p.width}×{p.height}</option>)}</select></label>:null}
    <small>应用会采用所选生成器的默认标量参数（含种子），再覆盖所填时长和分辨率；原提示词与媒体输入不变，冻结历史不变。</small></fieldset>;
  return <dialog ref={dialog} className="storyboard-batch-dialog" aria-label="批量准备视频" onCancel={e=>{e.preventDefault();leave(onClose);}}>
    <header><h2>批量准备视频</h2><button type="button" disabled={busy} onClick={()=>leave(onClose)}>返回创作</button></header>
    <p>1 选择镜头 → 2 正式预检 → 3 准备冻结批次 → 4 在队列明确启动。准备不会运行 GPU；修改镜头不会改写旧批次。</p>
    {error&&<p role="alert">{error}</p>}{shots?<ShotBulkConfigPanel projectId={projectId} shots={shots} configurationEditor={configurationEditor} onConfigureStage={generator?configure:undefined} videoOnly confirmPromptOverwrite onBulkPrompt={async(stage,ids,text)=>{if(stage!=="video")throw new Error("仅支持视频提示词");await productClient.creation.shotsPromptAssign(projectId,ids,text);}} onDirtyChange={reportDirty} onBusyChange={setBusy} onRefresh={reload} onOpenProductionQueue={id=>leave(()=>onQueue(id))} onOpenShot={id=>leave(()=>onShot(id))}/>:<p role="status">正在读取镜头…</p>}
  </dialog>;
}
