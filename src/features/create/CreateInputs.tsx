import { useEffect, useRef, useState } from "react";
import type { CreationContext } from "../../product/types";
import type { RecipeField } from "../../types/generation";
import type { CreateController } from "./CreateController";
import { assetIds, fieldLabel, generatorLabel, mediaKind, mediaValue } from "./createModel";
function textValue(value?: import("../../types/generation").DraftValue) { return value?.type === "string" ? value.value : ""; }
export function GeneratorPanel({ controller: c }: { controller: CreateController }) {
  return <section><h2>生成器</h2><label>选择生成器<select aria-label="选择生成器" value={c.selection} disabled={c.busy} onChange={e => c.chooseGenerator(e.target.value)}>
    <option value="">请选择</option>{c.generators.map((item, index) => <option key={item.selectionRef} value={item.selectionRef} disabled={!item.availability}>{generatorLabel(item, index)}{!item.availability ? "（不可用）" : ""}</option>)}
  </select></label>{c.generator?.availabilityReason && <p>{c.generator.availabilityReason}</p>}</section>;
}
export function PromptPanel({ controller: c }: { controller: CreateController }) {
  const [picker, setPicker] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (picker) dialog.current?.showModal(); }, [picker]);
  const fields = c.generator?.fields.filter(field => field.type === "textarea" && ["prompt", "negative_prompt"].includes(field.key)) ?? [];
  return <section><h2>提示词</h2>{fields.map(field => <label key={field.key}>{fieldLabel(field)}<textarea id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={textValue(c.values[field.key])} onChange={e => c.setValue(field.key, { type: "string", value: e.target.value })} rows={field.key === "prompt" ? 5 : 2} /></label>)}
    {fields.some(field => field.key === "prompt") && <button type="button" onClick={() => setPicker(true)}>选择提示词</button>}
    {picker && <dialog ref={dialog} className="create-picker" aria-label="选择提示词" onCancel={() => setPicker(false)}><h2>项目提示词</h2>
      {c.context?.promptChoices.length ? c.context.promptChoices.map((choice, index) => <button key={index} type="button" onClick={() => { c.setValue("prompt", { type: "string", value: choice.text }); setPicker(false); }}>{choice.name} · 版本 {choice.version}</button>) : <p>此项目暂无提示词。</p>}
      <button type="button" autoFocus onClick={() => setPicker(false)}>返回创作</button></dialog>}
  </section>;
}
function MediaField({ field, controller: c }: { field: RecipeField; controller: CreateController }) {
  const kind = mediaKind(field); const ids = assetIds(c.values[field.key]);
  const plural = ["images", "videos", "audios"].includes(field.type);
  const choices = c.context?.mediaInputs.filter(asset => asset.mediaKind === kind) ?? [];
  const set = (next: string[]) => next.length ? c.setValue(field.key, mediaValue(field, next)) : c.removeValue(field.key);
  return <label>{fieldLabel(field)}{("required" in field && field.required) ? "（必需）" : "（可选）"}
    <select id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={plural ? "" : ids[0] ?? ""} onChange={e => set(plural ? [...ids, e.target.value].filter(Boolean) : e.target.value ? [e.target.value] : [])}>
      <option value="">{plural ? "添加素材" : "未选择"}</option>{choices.filter(item => !plural || !ids.includes(item.id)).map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
    </select>
    {plural && <ol>{ids.map((id, index) => <li key={id}>{choices.find(item => item.id === id)?.name ?? "素材不可用"}<button type="button" aria-label={`${fieldLabel(field)}移除第${index + 1}项`} onClick={() => set(ids.filter(item => item !== id))}>移除</button>{index > 0 && <button type="button" onClick={() => { const next = [...ids]; [next[index - 1], next[index]] = [next[index], next[index - 1]]; set(next); }}>上移</button>}</li>)}</ol>}
    {plural && "maxItems" in field && <small>按顺序输入，允许 {field.minItems}–{field.maxItems} 项。</small>}
  </label>;
}
export function MediaInputPanel({ controller: c }: { controller: CreateController }) {
  return <section><h2>本次生成素材</h2>{c.generator?.fields.filter(field => mediaKind(field)).map(field => <MediaField key={field.key} field={field} controller={c} />)}<small>首帧、尾帧及视频/音频输入只属于本次草稿，不写入长期参考图。</small></section>;
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
  const common = new Set(["duration_seconds", "width", "height"]);
  const render = (field: RecipeField) => {
    const value = c.values[field.key];
    if (field.type === "seed") return <label key={field.key}>{fieldLabel(field)}<select id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={value?.type === "seed_fixed" ? "fixed" : "random"} onChange={e => c.setValue(field.key, e.target.value === "random" ? { type: "seed_random" } : { type: "seed_fixed", value: field.defaultValue ?? "0" })}><option value="random">随机</option><option value="fixed">固定</option></select>{value?.type === "seed_fixed" && <input aria-label="固定种子" value={value.value} onChange={e => c.setValue(field.key, { type: "seed_fixed", value: e.target.value })} />}</label>;
    if (field.type !== "integer" && field.type !== "number") return null;
    return <label key={field.key}>{fieldLabel(field)}<input id={`create-field-${field.key}`} aria-label={fieldLabel(field)} type="number" min={field.min} max={field.max} step={field.step ?? (field.type === "integer" ? 1 : "any")} value={value && (value.type === "integer" || value.type === "number") ? value.value : ""} onChange={e => { if (e.target.value === "") c.removeValue(field.key); else c.setValue(field.key, { type: field.type, value: Number(e.target.value) }); }} /></label>;
  };
  return <section><h2>参数</h2><div className="create-parameters">{fields.filter(field => common.has(field.key)).map(render)}</div><details><summary>高级参数</summary><div className="create-parameters">{fields.filter(field => !common.has(field.key)).map(render)}</div></details></section>;
}
