import { useEffect, useRef, useState } from "react";
import type { CreationContext } from "../../product/types";
import type { RecipeField } from "../../types/generation";
import type { CreateController } from "./CreateController";
import { assetIds, fieldLabel, generatorLabel, mediaKind, mediaValue } from "./createModel";
import { CreateMediaPreview } from "./CreateMediaPreview";
import { modeLabel } from "../../product/generatorPresentation";
function textValue(value?: import("../../types/generation").DraftValue) { return value?.type === "string" ? value.value : ""; }
export function GeneratorPanel({ controller: c }: { controller: CreateController }) {
  return <section aria-label="视频模式"><h2>视频模式</h2><label>选择生成器<select id="create-field-selectionRef" aria-label="选择生成器" value={c.selection} disabled={c.busy || c.videoInputs.busy} onChange={e => c.chooseGenerator(e.target.value)}>
    <option value="">请选择</option>{c.generators.map((item, index) => <option key={item.selectionRef} value={item.selectionRef} disabled={!item.availability}>{generatorLabel(item, index)}{!item.availability ? "（不可用）" : ""}</option>)}
  </select></label>{c.generator && <span className="create-state-pill">{modeLabel(c.generator)}</span>}{c.generator?.availabilityReason && <p>{c.generator.availabilityReason}</p>}</section>;
}
export function PromptPanel({ controller: c }: { controller: CreateController }) {
  const [picker, setPicker] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (picker) dialog.current?.showModal(); }, [picker]);
  const fields = c.generator?.fields.filter(field => field.type === "textarea" && ["prompt", "negative_prompt"].includes(field.key)) ?? [];
  return <section><h2>提示词</h2>{fields.map(field => <label key={field.key}>{fieldLabel(field)}<textarea id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={textValue(c.values[field.key])} onChange={e => c.setValue(field.key, { type: "string", value: e.target.value })} rows={field.key === "prompt" ? 5 : 2} /></label>)}
    {fields.some(field => field.key === "prompt") && <button type="button" onClick={() => setPicker(true)}>选择提示词</button>}
    {fields.some(field=>field.key==="prompt") && <button type="button" onClick={()=>c.openLibrary("prompts")}>在资源库查找更多提示词</button>}
    {picker && <dialog ref={dialog} className="create-picker" aria-label="选择提示词" onCancel={() => setPicker(false)}><h2>近期提示词</h2><p>此处显示当前项目最多20个近期提示词条目的最新版本。更早的提示词可在资源库中查找。</p>
      {c.context?.promptChoices.length ? c.context.promptChoices.map(choice => <button key={choice.promptVersionId} type="button" onClick={() => { c.applyPromptChoice(choice); setPicker(false); }}>{choice.name} · 版本 {choice.version}</button>) : <p>此项目暂无近期提示词。</p>}
      <button type="button" onClick={()=>{setPicker(false);c.openLibrary("prompts");}}>在资源库查找更多提示词</button>
      <button type="button" autoFocus onClick={() => setPicker(false)}>返回创作</button></dialog>}
  </section>;
}
function MediaField({ field, controller: c }: { field: RecipeField; controller: CreateController }) {
  const kind = mediaKind(field); const ids = assetIds(c.values[field.key]);
  const plural = ["images", "videos", "audios"].includes(field.type);
  const choices = (c.videoInputs.enabled ? c.videoInputs.view?.assets : c.context?.mediaInputs)?.filter(asset => asset.mediaKind === kind) ?? [];
  const set = (next: string[]) => next.length ? c.setValue(field.key, mediaValue(field, next)) : c.removeValue(field.key);
  return <div className="create-media-field"><label htmlFor={`create-field-${field.key}`}>{fieldLabel(field)}{("required" in field && field.required) ? "（必需）" : "（可选）"}{plural && ` · ${ids.length} 项`}</label>
    <select id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={plural ? "" : ids[0] ?? ""} onChange={e => set(plural ? [...ids, e.target.value].filter(Boolean) : e.target.value ? [e.target.value] : [])}>
      <option value="">{plural ? "添加素材" : "未选择"}</option>{choices.filter(item => !plural || !ids.includes(item.id)).map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
    </select>
    <ol>{ids.map((id, index) => { const asset = choices.find(item => item.id === id); return <li key={id}><strong>{asset?.name ?? "素材不可用"}</strong><div className="create-media-actions"><button type="button" aria-label={`${fieldLabel(field)}移除第${index + 1}项`} onClick={() => set(ids.filter(item => item !== id))}>移除</button>{plural && <><button type="button" aria-label={`${fieldLabel(field)}上移第${index + 1}项`} disabled={index === 0} onClick={() => { const next = [...ids]; [next[index - 1], next[index]] = [next[index], next[index - 1]]; set(next); }}>上移</button><button type="button" aria-label={`${fieldLabel(field)}下移第${index + 1}项`} disabled={index === ids.length - 1} onClick={() => { const next = [...ids]; [next[index], next[index + 1]] = [next[index + 1], next[index]]; set(next); }}>下移</button></>}</div>{asset && <InputPreview key={`${c.context?.projectId}:${c.selection}:${field.key}:${id}`} projectId={c.context!.projectId} asset={asset} />}{!asset && <small role="status">请刷新输入并重新选择；不会自动替换素材。</small>}</li>; })}</ol>
    {plural && "maxItems" in field && <small>按顺序输入，允许 {field.minItems}–{field.maxItems} 项。</small>}
  </div>;
}
function InputPreview({ projectId, asset }: { projectId: string; asset: { id: string; name: string; mediaKind: "image" | "video" | "audio" } }) {
  const [open, setOpen] = useState(false);
  return <details className="create-input-preview" onToggle={e => setOpen(e.currentTarget.open)}><summary>预览{asset.name}</summary>{open && <CreateMediaPreview key={`${projectId}:${asset.id}`} projectId={projectId} asset={asset} />}</details>;
}
export function MediaInputPanel({ controller: c }: { controller: CreateController }) {
  const fields=c.generator?.fields.filter(field=>mediaKind(field)) ?? [];
  if (c.videoInputs.enabled) return <section aria-label="镜头视频输入"><h2>镜头视频输入</h2><p>输入按当前生成器独立保存；导入不是生图结果，不会自动填充首尾帧。</p>
    <fieldset disabled={c.busy || c.videoInputs.loading || c.videoInputs.busy || !c.videoInputs.view}>{fields.map(field => <MediaField key={field.key} field={field} controller={c} />)}
      {!fields.length && <p>T2V 无需图片输入。</p>}
      <button type="button" onClick={()=>void c.videoInputs.importAssets(false)}>导入图片、视频或音频</button><button type="button" onClick={()=>void c.videoInputs.importAssets(true)}>导入图片文件夹</button>
      <button type="button" onClick={()=>void c.videoInputs.save()} disabled={!c.videoInputs.dirty}>保存视频输入</button>
    </fieldset><button type="button" disabled={c.busy || c.videoInputs.busy} onClick={()=>void c.videoInputs.refresh()}>刷新已保存输入</button>
    {c.generator?.mode?.toUpperCase() === "REF2VA" && <small>混合参考：图片最多9项、视频3项、音频3项，共12项。可仅用合法视频；纯音频不支持。时长和媒体约束由统一预检确认。</small>}
    {c.videoInputs.loading && <p>正在加载当前 Recipe 输入…</p>}{c.videoInputs.dirty ? <p>输入尚未保存。</p> : c.videoInputs.view && <small>当前 Recipe 输入已保存</small>}{c.videoInputs.importReceipt && <p role="status">{c.videoInputs.importReceipt}</p>}{c.videoInputs.error && <p role="alert">{c.videoInputs.error}</p>}</section>;
  return <section><h2>本次生成素材</h2>{fields.map(field => <MediaField key={field.key} field={field} controller={c} />)}{fields.length>0&&<p>下拉列表显示当前项目近期100项素材，并包含当前镜头明确关联的素材。更早的素材可从资源库查找。</p>}{fields.some(field=>mediaKind(field)==="image")&&<button type="button" onClick={()=>c.openLibrary("images")}>在资源库查找更多图片</button>}{fields.some(field=>mediaKind(field)==="video")&&<button type="button" onClick={()=>c.openLibrary("videos")}>在资源库查找更多视频</button>}<small>这些媒体输入只属于本次草稿，不会修改镜头长期参考图。</small></section>;
}
export function ReferencePanel({ context, save, busy }: { context: CreationContext; save: (ids: string[]) => unknown; busy: boolean }) {
  const current = context.selectedShot?.referenceAssetIds ?? [];
  const [ids, setIds] = useState(current);
  useEffect(() => { setIds(current); }, [context]);
  return <details><summary>镜头长期参考图</summary><p>仅保存图片；不会替代明确的首帧、尾帧或视频输入。</p>
    {context.mediaInputs.filter(asset => asset.mediaKind === "image").map(asset => <label className="create-reference" key={asset.id}><input type="checkbox" checked={ids.includes(asset.id)} onChange={e => setIds(e.target.checked ? [...ids, asset.id] : ids.filter(id => id !== asset.id))} />{asset.name}</label>)}
    <button type="button" disabled={busy} onClick={() => save(ids)}>保存长期参考图</button></details>;
}
export function ParameterPanel({ controller: c }: { controller: CreateController }) {
  const fields = c.generator?.fields.filter(field => ["integer", "number", "seed"].includes(field.type)) ?? [];
  const presets = c.generator?.resolutionPresets ?? [];
  const width = c.values.width; const height = c.values.height;
  const selectedPreset = presets.find(preset => width?.type === "integer" && height?.type === "integer" && preset.width === width.value && preset.height === height.value);
  const common = new Set(["duration_seconds", "width", "height"]);
  const render = (field: RecipeField) => {
    const value = c.values[field.key];
    if (field.type === "seed") return <label key={field.key}>{fieldLabel(field)}<select id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={value?.type === "seed_fixed" ? "fixed" : "random"} onChange={e => c.setValue(field.key, e.target.value === "random" ? { type: "seed_random" } : { type: "seed_fixed", value: field.defaultValue ?? "0" })}><option value="random">随机</option><option value="fixed">固定</option></select>{value?.type === "seed_fixed" && <input aria-label="固定种子" value={value.value} onChange={e => c.setValue(field.key, { type: "seed_fixed", value: e.target.value })} />}</label>;
    if (field.type !== "integer" && field.type !== "number") return null;
    return <label key={field.key}>{fieldLabel(field)}<input id={`create-field-${field.key}`} aria-label={fieldLabel(field)} type="number" min={field.min} max={field.max} step={field.step ?? (field.type === "integer" ? 1 : "any")} value={value && (value.type === "integer" || value.type === "number") ? value.value : ""} onChange={e => { if (e.target.value === "") c.removeValue(field.key); else c.setValue(field.key, { type: field.type, value: Number(e.target.value) }); }} /></label>;
  };
  return <section><h2>参数</h2>{presets.length > 0 && <label>视频分辨率<select id="create-field-resolution" aria-label="视频分辨率" value={selectedPreset?.id ?? "existing"} disabled={c.busy} onChange={e => c.chooseResolution(e.target.value)}>{!selectedPreset && <option value="existing" disabled>保留当前尺寸；可选择内置规格</option>}{presets.map(preset => <option key={preset.id} value={preset.id}>{preset.label} · {preset.width} × {preset.height}</option>)}</select><small>基于 H3-Base 官方短边768及32像素网格的常用规格。2K需独立再生成流程，此处不提供。高分辨率需要更多显存。</small></label>}<div className="create-parameters">{fields.filter(field => common.has(field.key) && !(presets.length && ["width", "height"].includes(field.key))).map(render)}</div><details><summary>高级参数</summary><div className="create-parameters">{fields.filter(field => !common.has(field.key)).map(render)}</div></details></section>;
}
