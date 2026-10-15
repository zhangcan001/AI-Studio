import { useEffect, useRef, useState } from "react";
import type { CreationContext } from "../../product/types";
import type { RecipeField } from "../../types/generation";
import type { CreateController } from "./CreateController";
import { fieldLabel, generatorLabel } from "./createModel";
import { modeLabel } from "../../product/generatorPresentation";
const VIDEO_MODE_CARDS = [
  { label: "文生视频", hint: "通过文字描述生成视频", icon: "Aa" },
  { label: "图生视频", hint: "基于图片生成动态视频", icon: "▣" },
  { label: "首尾帧", hint: "指定首尾帧生成过渡视频", icon: "◧" },
  { label: "混合参考", hint: "结合多种参考生成视频", icon: "✣" },
] as const;
export function GeneratorPanel({ controller: c }: { controller: CreateController }) {
  const currentMode = c.generator ? modeLabel(c.generator) : undefined;
  const pick = (label: string) => {
    const options = c.generators.filter(item => item.mediaKind === "video" && modeLabel(item) === label);
    const target = options.find(item => item.selectionRef === c.selection) ?? options.find(item => item.availability && item.recommended) ?? options.find(item => item.availability);
    if (target && target.selectionRef !== c.selection) c.chooseGenerator(target.selectionRef);
  };
  return <section aria-label="视频模式" className="create-mode-section"><h2>生成设置</h2>
    <div className="create-mode-cards" role="radiogroup" aria-label="视频模式卡片">{VIDEO_MODE_CARDS.map(card => {
      const available = c.generators.some(item => item.mediaKind === "video" && item.availability && modeLabel(item) === card.label);
      return <button key={card.label} type="button" role="radio" aria-checked={currentMode === card.label} className="create-mode-card" disabled={c.busy || c.videoInputs.busy || !available} title={available ? card.hint : "当前项目没有可用的此模式生成器"} onClick={() => pick(card.label)}>
        <span aria-hidden="true" className="create-mode-icon">{card.icon}</span><strong>{card.label}</strong><small>{available ? card.hint : "暂无可用生成器"}</small></button>;
    })}</div>
    <label className="create-generator-select">生成器<select id="create-field-selectionRef" aria-label="选择生成器" value={c.selection} disabled={c.busy || c.videoInputs.busy} onChange={e => c.chooseGenerator(e.target.value)}>
    <option value="">请选择</option>{c.generators.filter(item => item.mediaKind === "video" || item.selectionRef === c.selection).map((item, index) => <option key={item.selectionRef} value={item.selectionRef} disabled={!item.availability}>{generatorLabel(item, index)}{!item.availability ? "（不可用）" : ""}</option>)}
  </select></label>{c.generator?.availabilityReason && <p>{c.generator.availabilityReason}</p>}</section>;
}
/** Recent prompt picker (moved from the old prompt panel). Applies through the existing prompt-choice path. */
export function PromptPickerDialog({ controller: c, onClose }: { controller: CreateController; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="create-picker" aria-label="选择提示词" onCancel={onClose}><h2>近期提示词</h2><p>此处显示当前项目最多20个近期提示词条目的最新版本。更早的提示词可在资源库中查找。</p>
    {c.context?.promptChoices.length ? c.context.promptChoices.map(choice => <button key={choice.promptVersionId} type="button" onClick={() => { c.applyPromptChoice(choice); onClose(); }}>{choice.name} · 版本 {choice.version}</button>) : <p>此项目暂无近期提示词。</p>}
    <button type="button" onClick={() => { onClose(); c.openLibrary("prompts"); }}>在资源库查找更多提示词</button>
    <button type="button" autoFocus onClick={onClose}>返回创作</button></dialog>;
}
/** Shot long-term reference images (shot-level setting, not a generator input). */
export function ReferencePanel({ context, save, busy }: { context: CreationContext; save: (ids: string[]) => unknown; busy: boolean }) {
  const current = context.selectedShot?.referenceAssetIds ?? [];
  const [ids, setIds] = useState(current);
  useEffect(() => { setIds(current); }, [context]);
  return <details><summary>镜头长期参考图</summary><p>仅保存图片；不会替代明确的首帧、尾帧或视频输入。</p>
    {context.mediaInputs.filter(asset => asset.mediaKind === "image").map(asset => <label className="create-reference" key={asset.id}><input type="checkbox" checked={ids.includes(asset.id)} onChange={e => setIds(e.target.checked ? [...ids, asset.id] : ids.filter(id => id !== asset.id))} />{asset.name}</label>)}
    <button type="button" disabled={busy} onClick={() => save(ids)}>保存长期参考图</button></details>;
}
/** One numeric/seed parameter control bound to the draft (moved from the old parameter panel). */
export function ParamField({ field, controller: c }: { field: RecipeField; controller: CreateController }) {
    const value = c.values[field.key];
    if (field.type === "seed") return <label key={field.key}>{fieldLabel(field)}<select id={`create-field-${field.key}`} aria-label={fieldLabel(field)} value={value?.type === "seed_fixed" ? "fixed" : "random"} onChange={e => c.setValue(field.key, e.target.value === "random" ? { type: "seed_random" } : { type: "seed_fixed", value: field.defaultValue ?? "0" })}><option value="random">随机</option><option value="fixed">固定</option></select>{value?.type === "seed_fixed" && <input aria-label="固定种子" value={value.value} onChange={e => c.setValue(field.key, { type: "seed_fixed", value: e.target.value })} />}</label>;
    if (field.type !== "integer" && field.type !== "number") return null;
    return <label key={field.key}>{fieldLabel(field)}<input id={`create-field-${field.key}`} aria-label={fieldLabel(field)} type="number" min={field.min} max={field.max} step={field.step ?? (field.type === "integer" ? 1 : "any")} value={value && (value.type === "integer" || value.type === "number") ? value.value : ""} onChange={e => { if (e.target.value === "") c.removeValue(field.key); else c.setValue(field.key, { type: field.type, value: Number(e.target.value) }); }} /></label>;
}
type ResolutionPreset = NonNullable<NonNullable<CreateController["generator"]>["resolutionPresets"]>[number];
/** 视频分辨率 picker over the backend H3 megapixel presets; writes width/height atomically via chooseResolution. */
export function ResolutionPicker({ controller: c, presets, current }: { controller: CreateController; presets: ResolutionPreset[]; current?: ResolutionPreset }) {
  // Show megapixel options for the current aspect; the separate aspect control switches ratio.
  const scoped = current ? presets.filter(p => Math.abs(p.width / p.height - current.width / current.height) < 0.03) : presets;
  const base = scoped.length ? scoped : presets;
  const list = current && !base.some(p => p.id === current.id) ? [current, ...base] : base;
  return <label className="composer-pick" title="分辨率">▣<select id="create-field-resolution" aria-label="视频分辨率" value={current?.id ?? ""} disabled={c.busy} onChange={e => c.chooseResolution(e.target.value)}>{!current && <option value="" disabled>自定义</option>}{list.map(p => <option key={p.id} value={p.id}>{p.label || `${p.width}×${p.height}`}</option>)}</select></label>;
}
