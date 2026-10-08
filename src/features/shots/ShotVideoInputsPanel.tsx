import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import type { InputAsset, ShotVideoInputScope, VideoInputView, VideoInputAsset, VideoInputKey } from "../../types/shotVideoInput";
import type { RecipeViewModel } from "../../types/generation";
import { normalizeProductError } from "../../product/errors";

/** An OCC edit form for persisted bindings, not an alternate generation draft/executor. */
export function ShotVideoInputsPanel({ scope, recipe, onSaved, onReadyChange }: { scope: ShotVideoInputScope; recipe: RecipeViewModel; onSaved?: () => void; onReadyChange?: (owner: string, ready: boolean) => void }) {
  const owner = JSON.stringify(scope); const live = useRef(owner); live.current = owner;
  const epoch = useRef(0); const lock = useRef(false);
  const [set, setSet] = useState<VideoInputView | null>(null), [inputs, setInputs] = useState<VideoInputAsset[]>([]);
  const [assets, setAssets] = useState<InputAsset[]>([]), [busy, setBusy] = useState(true), [error, setError] = useState<string>(), [loaded, setLoaded] = useState(false);
  const fields = recipe.fields.filter(f=>["image","images","video","videos","audio","audios"].includes(f.type));
  const selection = {projectId:scope.projectId,shotId:scope.shotId,selectionRef:recipe.selectionRef??""};
  const ordered = (items: VideoInputAsset[]) => JSON.stringify([...items].sort((a,b)=>a.inputKey.localeCompare(b.inputKey)||a.ordinal-b.ordinal));
  const dirty = loaded && ordered(inputs) !== ordered(set?.inputs ?? []);
  const ready = loaded && !busy && !error && !dirty;
  useEffect(()=>{onReadyChange?.(owner,ready);},[owner,ready,onReadyChange]);
  async function load() {
    const own=owner, token=++epoch.current; setBusy(true); setLoaded(false); setError(undefined);
    try {
      const current=await productClient.creation.inputsGet(selection);
      if (live.current!==own || token!==epoch.current) return;
      if(current.selectionRef!==selection.selectionRef)throw new Error("当前生成器输入身份不匹配");
      setSet(current); setInputs(current.inputs); setAssets(current.assets); setLoaded(true);
    } catch(e) { if(live.current===own && token===epoch.current)setError(normalizeProductError(e).message); }
    finally { if(live.current===own && token===epoch.current)setBusy(false); }
  }
  useEffect(()=>{lock.current=false;setSet(null);setInputs([]);setAssets([]);void load();return()=>{++epoch.current;};},[owner]);
  async function save() {
    if (!loaded || lock.current) return; lock.current=true;setBusy(true);setError(undefined);
    const own=owner, token=epoch.current;
    const combination=combinationMessage(inputs); if(combination){setError(combination); lock.current=false; setBusy(false); return;}
    try { const current=await productClient.creation.inputsSave({selection,expected:set?.token??null,inputs}); if(live.current===own && token===epoch.current){setSet(current);onSaved?.();} }
    catch(e) {if(live.current===own && token===epoch.current)setError(normalizeProductError(e).message);}
    finally {if(live.current===own && token===epoch.current){lock.current=false;setBusy(false);}}
  }
  async function importFiles(folder:boolean) {
    if(lock.current)return;lock.current=true;setBusy(true);setError(undefined);const own=owner,token=epoch.current;
    try {const result=await productClient.creation.assetsImport(scope.projectId,folder);if(live.current!==own||token!==epoch.current)return;
      setAssets(old=>[...old,...result.imported.filter(a=>!old.some(b=>b.id===a.id))]);
      if(result.failed.length)setError(result.failed.map(f=>`${f.displayName}：${f.error}`).join("；"));
    }catch(e){if(live.current===own&&token===epoch.current)setError(normalizeProductError(e).message);}
    finally{if(live.current===own&&token===epoch.current){lock.current=false;setBusy(false);}}
  }
  const COMBINATION = "参考图最多 9 个、参考视频最多 3 个、参考音频最多 3 个，合计不超过 12 个，且不能只有音频。";
  const slotMax: Record<string, number> = {reference_images:9, reference_videos:3, reference_audios:3};
  function counts(items: VideoInputAsset[]) {return {images:items.filter(i=>i.inputKey==="reference_images").length,videos:items.filter(i=>i.inputKey==="reference_videos").length,audios:items.filter(i=>i.inputKey==="reference_audios").length};}
  function combinationMessage(items: VideoInputAsset[]) {const {images,videos,audios}=counts(items); if(images>9||videos>3||audios>3||images+videos+audios>12||(audios>0&&images+videos===0)) return COMBINATION;}
  function replace(key: string, ids: string[]) {const max=slotMax[key]; if(max!==undefined && ids.length>max){setError(COMBINATION); return;} const next=[...inputs.filter(i=>i.inputKey!==key),...ids.map((assetId,ordinal)=>({inputKey:key as VideoInputKey,assetId,ordinal}))]; if(counts(next).images+counts(next).videos+counts(next).audios>12){setError(COMBINATION); return;} setInputs(next);}
  return <section aria-label="正式视频输入"><h3>正式视频输入</h3><p>按当前 Recipe 独立保存。导入不代表图片生成完成；生成仅使用已保存输入。</p>
    <fieldset disabled={busy||!loaded}>{fields.map(field=>{const ids=inputs.filter(i=>i.inputKey===field.key).sort((a,b)=>a.ordinal-b.ordinal).map(i=>i.assetId),plural=["images","videos","audios"].includes(field.type),kind=field.type.startsWith("image")?"image":field.type.startsWith("video")?"video":"audio";
      return <label key={field.key}>{field.label}<select aria-label={field.label} value={plural?"":ids[0]??""} onChange={e=>replace(field.key,plural?[...ids,e.target.value].filter(Boolean):e.target.value?[e.target.value]:[])}><option value="">{plural?"添加素材":"未选择"}</option>{assets.filter(a=>a.mediaKind===kind&&(!plural||!ids.includes(a.id))).map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</select>
      {plural&&<ol>{ids.map((id,index)=><li key={`${id}:${index}`}>{assets.find(a=>a.id===id)?.name??"素材需重新选择"}<button type="button" onClick={()=>replace(field.key,ids.filter((_,i)=>i!==index))}>移除</button>{index>0&&<button type="button" onClick={()=>{const next=[...ids];[next[index-1],next[index]]=[next[index],next[index-1]];replace(field.key,next);}}>上移</button>}</li>)}</ol>}</label>;})}
      {!fields.length&&<p>T2V 无需图片输入。</p>}<button type="button" onClick={()=>void save()}>保存视频输入</button><button type="button" onClick={()=>void importFiles(false)}>导入媒体素材</button><button type="button" onClick={()=>void importFiles(true)}>导入图片文件夹</button>
    </fieldset><button type="button" disabled={busy} onClick={()=>void load()}>刷新输入</button>{dirty&&<p>输入尚未保存，保存后才能生成。</p>}{error&&<p role="alert">{error}</p>}</section>;
}
