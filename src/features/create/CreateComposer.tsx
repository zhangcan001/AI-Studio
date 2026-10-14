import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
type CreationAsset = { id: string; name: string; mediaKind: string; thumbnailBytes?: number[] | null };
import type { RecipeField } from "../../types/generation";
import type { CreateController } from "./CreateController";
import { assetIds, mediaKind, mediaValue } from "./createModel";
import { modeLabel } from "../../product/generatorPresentation";

/** One bound image reference; `n` is its 1-based "Image N" label in submit order. */
export interface ComposerReference { n: number; fieldKey: string; assetId: string; asset?: CreationAsset }
const VIDEO_MODES = ["文生视频", "图生视频", "首尾帧", "混合参考"] as const;
const TOKEN = /Image (\d+)/g;
/** MiniMax H3 open weights: 4–15 s at 24 fps (https://huggingface.co/MiniMaxAI/MiniMax-H3). Narrows the picker only; the recipe field still validates. */
const H3_DURATION = { min: 4, max: 15 } as const;
/** Local H3-Base canvas (768 short edge, multiple of 32). Matches src-tauri h3_resolution.rs / ComfyUI H3 native docs. 2K is a separate regenerate pass and is not listed. */
const H3_BASE = [
  { id: "landscape", label: "768p", width: 1344, height: 768 },
  { id: "portrait", label: "768p", width: 768, height: 1344 },
  { id: "square", label: "768p", width: 768, height: 768 },
  { id: "landscape-4-3", label: "768p", width: 1024, height: 768 },
  { id: "portrait-3-4", label: "768p", width: 768, height: 1024 },
] as const;

/** Image N follows the order of the bound image inputs (field order, then item order), which is what the generator receives. */
export function composerReferences(fields: RecipeField[], values: CreateController["values"], choices: CreationAsset[]): ComposerReference[] {
  const out: ComposerReference[] = [];
  for (const field of fields) if (mediaKind(field) === "image") for (const id of assetIds(values[field.key])) out.push({ n: out.length + 1, fieldKey: field.key, assetId: id, asset: choices.find(a => a.id === id) });
  return out;
}
function gcd(a: number, b: number): number { return b ? gcd(b, a % b) : a; }
export function aspectOf(w: number, h: number) { const g = gcd(w, h) || 1; const r = `${w / g}:${h / g}`; const known: Record<string, string> = { "16:9": "16:9", "9:16": "9:16", "1:1": "1:1", "4:3": "4:3", "3:4": "3:4", "21:9": "21:9" }; if (known[r]) return r; const q = w / h; return Object.keys(known).find(k => { const [a, b] = k.split(":").map(Number); return Math.abs(a / b - q) < 0.02; }) ?? r; }

function useThumbUrls(refs: ComposerReference[]) {
  const key = refs.map(r => `${r.assetId}:${r.asset?.thumbnailBytes?.length ?? 0}`).join("|");
  const urls = useMemo(() => {
    const map = new Map<string, string>();
    if (typeof URL.createObjectURL !== "function") return map;
    for (const r of refs) if (r.asset?.thumbnailBytes?.length && !map.has(r.assetId)) map.set(r.assetId, URL.createObjectURL(new Blob([new Uint8Array(r.asset.thumbnailBytes)])));
    return map;
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => { for (const u of urls.values()) URL.revokeObjectURL?.(u); }, [urls]);
  return urls;
}

function chipNode(ref: ComposerReference, url?: string) {
  const chip = document.createElement("span");
  chip.className = "composer-chip"; chip.contentEditable = "false"; chip.dataset.ref = String(ref.n);
  if (url) { const img = document.createElement("img"); img.src = url; img.alt = ""; chip.appendChild(img); }
  const label = document.createElement("span"); label.textContent = `Image ${ref.n}`; chip.appendChild(label);
  return chip;
}
function build(root: HTMLElement, value: string, refs: ComposerReference[], urls: Map<string, string>) {
  root.replaceChildren();
  let last = 0;
  for (const m of value.matchAll(TOKEN)) {
    const ref = refs.find(r => r.n === Number(m[1]));
    if (!ref) continue;
    if (m.index! > last) root.appendChild(document.createTextNode(value.slice(last, m.index)));
    root.appendChild(chipNode(ref, urls.get(ref.assetId)));
    last = m.index! + m[0].length;
  }
  if (last < value.length) root.appendChild(document.createTextNode(value.slice(last)));
}
function serialize(node: Node): string {
  let out = "";
  node.childNodes.forEach(child => {
    if (child.nodeType === Node.TEXT_NODE) out += child.textContent ?? "";
    else if (child instanceof HTMLElement) {
      if (child.dataset.ref) out += `Image ${child.dataset.ref}`;
      else if (child.tagName === "BR") out += "\n";
      else { if (child.tagName === "DIV" && out && !out.endsWith("\n")) out += "\n"; out += serialize(child); }
    }
  });
  return out;
}

/** Composer card: reference strip, prompt with inline Image N chips, and a parameter toolbar. It only edits the existing draft and calls the existing generate path. */
export function CreateComposer({ controller: c }: { controller: CreateController }) {
  const fields = c.generator?.fields ?? [];
  const promptField = fields.find(f => f.type === "textarea" && f.key === "prompt");
  const imageFields = fields.filter(f => mediaKind(f) === "image");
  const choices = ((c.videoInputs.enabled ? c.videoInputs.view?.assets : c.context?.mediaInputs) ?? []).filter(a => a.mediaKind === "image");
  const refs = composerReferences(fields, c.values, choices);
  const urls = useThumbUrls(refs);
  const prompt = c.values.prompt?.type === "string" ? c.values.prompt.value : "";
  const editor = useRef<HTMLDivElement>(null);
  const emitted = useRef<string | null>(null);
  const caret = useRef<Range | null>(null);
  const [picker, setPicker] = useState<"insert" | "add" | null>(null);
  const refKey = refs.map(r => `${r.n}:${r.assetId}:${urls.has(r.assetId)}`).join("|");
  const inputsLocked = c.busy || c.videoInputs.busy || c.videoInputs.loading || (c.videoInputs.enabled && !c.videoInputs.view);

  useLayoutEffect(() => {
    const root = editor.current; if (!root) return;
    if (emitted.current === prompt && root.dataset.refs === refKey) return;
    build(root, prompt, refs, urls); root.dataset.refs = refKey; emitted.current = prompt;
  }, [prompt, refKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const emit = () => { const root = editor.current; if (!root || !promptField) return; const next = serialize(root); emitted.current = next; c.setValue("prompt", { type: "string", value: next }); };
  const saveCaret = () => { const sel = window.getSelection(); if (sel?.rangeCount && editor.current?.contains(sel.anchorNode)) caret.current = sel.getRangeAt(0).cloneRange(); };
  const insert = (ref: ComposerReference) => {
    const root = editor.current; if (!root) return;
    const chip = chipNode(ref, urls.get(ref.assetId)); const space = document.createTextNode(" ");
    const range = caret.current && root.contains(caret.current.startContainer) ? caret.current : null;
    if (range) { range.deleteContents(); range.insertNode(space); range.insertNode(chip); } else root.append(chip, space);
    const sel = window.getSelection(); if (sel) { const r = document.createRange(); r.setStartAfter(space); r.collapse(true); sel.removeAllRanges(); sel.addRange(r); caret.current = r.cloneRange(); }
    setPicker(null); emit(); root.focus();
  };
  const setField = (field: RecipeField, ids: string[]) => ids.length ? c.setValue(field.key, mediaValue(field, ids)) : c.removeValue(field.key);
  const target = imageFields.find(f => { const ids = assetIds(c.values[f.key]); const plural = f.type === "images"; return plural ? !("maxItems" in f) || ids.length < (f.maxItems ?? Infinity) : !ids.length; });
  const add = (id: string) => { if (!target) return; const ids = assetIds(c.values[target.key]); setField(target, target.type === "images" ? [...ids, id] : [id]); setPicker(null); };
  const remove = (ref: ComposerReference) => { const f = imageFields.find(x => x.key === ref.fieldKey); if (f) setField(f, assetIds(c.values[f.key]).filter(id => id !== ref.assetId)); };

  // Mode (video only: image generators are retired).
  const currentMode = c.generator ? modeLabel(c.generator) : undefined;
  const pickMode = (label: string) => {
    const options = c.generators.filter(g => g.mediaKind === "video" && modeLabel(g) === label);
    const next = options.find(g => g.selectionRef === c.selection) ?? options.find(g => g.availability && g.recommended) ?? options.find(g => g.availability);
    if (next && next.selectionRef !== c.selection) c.chooseGenerator(next.selectionRef);
  };
  // Resolution / aspect: prefer generator presets; when they include H3 Base (768 short edge), expose the fixed built-ins only.
  const presets = c.generator?.resolutionPresets ?? [];
  const h3Presets = presets.filter(p => H3_BASE.some(b => b.width === p.width && b.height === p.height));
  const pickerPresets = h3Presets.length ? H3_BASE.map(b => h3Presets.find(p => p.width === b.width && p.height === b.height)!).filter(Boolean) : presets;
  const w = c.values.width, h = c.values.height;
  const current = pickerPresets.find(p => w?.type === "integer" && h?.type === "integer" && p.width === w.value && p.height === h.value)
    ?? presets.find(p => w?.type === "integer" && h?.type === "integer" && p.width === w.value && p.height === h.value);
  const aspects = [...new Set(pickerPresets.map(p => aspectOf(p.width, p.height)))];
  const aspect = current ? aspectOf(current.width, current.height) : undefined;
  const sameAspect = pickerPresets.filter(p => !aspect || aspectOf(p.width, p.height) === aspect);
  const chooseAspect = (a: string) => { const best = pickerPresets.find(p => aspectOf(p.width, p.height) === a); if (best) c.chooseResolution(best.id); };
  const durationField = fields.find(f => f.key === "duration_seconds" && (f.type === "integer" || f.type === "number"));
  const dv = c.values.duration_seconds; const duration = dv?.type === "integer" || dv?.type === "number" ? dv.value : undefined;
  const durationOptions = (() => { if (!durationField || !("min" in durationField) || durationField.min === undefined || durationField.max === undefined) return null; const step = Number(durationField.step ?? 1) || 1; const lo = Math.max(durationField.min, H3_DURATION.min), hi = Math.min(durationField.max, H3_DURATION.max); const [from, to] = lo <= hi ? [lo, hi] : [durationField.min, durationField.max]; const out: number[] = []; for (let v = from; v <= to && out.length < 40; v += step) out.push(Number(v.toFixed(3))); return out.length < 40 ? out : null; })();
  const canSubmit = !c.busy && Boolean(c.generator?.availability) && c.videoInputs.ready && c.readinessState === "READY";

  if (!c.generator) return null;
  return <section className="create-composer" aria-label="创作输入框">
    {(imageFields.length > 0) && <div className="composer-refs" aria-label="参考图">
      {refs.map(ref => <div key={`${ref.fieldKey}:${ref.assetId}`} className="composer-ref">
        <button type="button" className="composer-ref-thumb" title={`插入 Image ${ref.n}${ref.asset ? ` · ${ref.asset.name}` : ""}`} aria-label={`插入 Image ${ref.n}`} disabled={!promptField} onMouseDown={e => e.preventDefault()} onClick={() => insert(ref)}>
          {urls.get(ref.assetId) ? <img src={urls.get(ref.assetId)} alt="" /> : <span>{ref.n}</span>}<small>Image {ref.n}</small></button>
        <button type="button" className="composer-ref-remove" aria-label={`移除 Image ${ref.n}`} disabled={inputsLocked} onClick={() => remove(ref)}>×</button>
      </div>)}
      {!refs.length && <small className="composer-hint">点击 + 添加参考图；在提示词中输入 @ 插入 Image 标签。</small>}
    </div>}
    {promptField && <div className="composer-prompt">
      <div ref={editor} className="composer-editor" contentEditable={!c.busy} suppressContentEditableWarning role="textbox" aria-multiline="true" aria-label="提示词编辑器" data-placeholder="描述画面；输入 @ 引用参考图"
        onInput={emit} onKeyUp={saveCaret} onMouseUp={saveCaret} onBlur={saveCaret}
        onKeyDown={e => { if (e.key === "@" && refs.length) { e.preventDefault(); saveCaret(); setPicker("insert"); } else if (e.key === "Escape") setPicker(null); }}
        onPaste={e => { e.preventDefault(); document.execCommand("insertText", false, e.clipboardData.getData("text/plain")); }} />
      <textarea id="create-field-prompt" className="composer-source" tabIndex={-1} aria-label="提示词" value={prompt} onChange={e => c.setValue("prompt", { type: "string", value: e.target.value })} />
    </div>}
    {picker && <div className="composer-picker" role="listbox" aria-label={picker === "insert" ? "插入参考图" : "添加参考图"}>
      {picker === "insert" ? refs.map(ref => <button type="button" role="option" aria-selected="false" key={ref.n} onMouseDown={e => e.preventDefault()} onClick={() => insert(ref)}>{urls.get(ref.assetId) && <img src={urls.get(ref.assetId)} alt="" />}Image {ref.n}{ref.asset ? ` · ${ref.asset.name}` : ""}</button>)
        : <>{choices.filter(a => !refs.some(r => r.assetId === a.id)).map(a => <button type="button" role="option" aria-selected="false" key={a.id} onClick={() => add(a.id)}>{a.name}</button>)}
          {c.videoInputs.enabled && <button type="button" onClick={() => { setPicker(null); void c.videoInputs.importAssets(false); }}>从电脑导入…</button>}
          <button type="button" onClick={() => { setPicker(null); c.openLibrary("images"); }}>在资源库查找图片</button></>}
      <button type="button" className="composer-picker-close" onClick={() => setPicker(null)}>关闭</button>
    </div>}
    <div className="composer-toolbar">
      <button type="button" className="composer-icon" aria-label="添加参考图" title={target ? "添加参考图" : imageFields.length ? "参考图已满" : "当前模式不需要参考图"} disabled={!target || inputsLocked} onClick={() => setPicker(picker === "add" ? null : "add")}>+</button>
      <div className="composer-segment" role="group" aria-label="生成类型">
        <button type="button" aria-pressed="true">视频</button>
        <select aria-label="视频模式" value={currentMode ?? ""} disabled={c.busy || c.videoInputs.busy} onChange={e => pickMode(e.target.value)}>
          {!VIDEO_MODES.includes(currentMode as never) && <option value={currentMode ?? ""}>{currentMode ?? "选择模式"}</option>}
          {VIDEO_MODES.map(m => <option key={m} value={m} disabled={!c.generators.some(g => g.mediaKind === "video" && g.availability && modeLabel(g) === m)}>{m}</option>)}
        </select>
      </div>
      {c.videoInputs.enabled && c.videoInputs.dirty && <button type="button" className="composer-save" onClick={() => void c.videoInputs.save()}>保存输入</button>}
      <span className="composer-spacer" />
      {pickerPresets.length > 0 && <label className="composer-pick" title="分辨率">▣<select aria-label="分辨率" value={current?.id ?? ""} disabled={c.busy} onChange={e => c.chooseResolution(e.target.value)}>{!current && <option value="" disabled>自定义</option>}{sameAspect.map(p => <option key={p.id} value={p.id}>{Math.min(p.width, p.height)}p · {p.width}×{p.height}</option>)}</select></label>}
      {durationField && <label className="composer-pick" title="时长">◷{durationOptions
        ? <select aria-label="时长" value={duration ?? ""} disabled={c.busy} onChange={e => c.setValue("duration_seconds", { type: durationField.type as "integer", value: Number(e.target.value) })}>{duration === undefined && <option value="" disabled>时长</option>}{durationOptions.map(v => <option key={v} value={v}>{v}s</option>)}</select>
        : <input aria-label="时长" type="number" value={duration ?? ""} disabled={c.busy} onChange={e => e.target.value === "" ? c.removeValue("duration_seconds") : c.setValue("duration_seconds", { type: durationField.type as "integer", value: Number(e.target.value) })} />}</label>}
      {aspects.length > 0 && <label className="composer-pick" title="画面比例">▭<select aria-label="画面比例" value={aspect ?? ""} disabled={c.busy} onChange={e => chooseAspect(e.target.value)}>{!aspect && <option value="" disabled>比例</option>}{aspects.map(a => <option key={a} value={a}>{a}</option>)}</select></label>}
      <button type="button" className="composer-submit" aria-label="提交生成" title={canSubmit ? "写入生产队列并生成" : "请先完成准备检查"} disabled={!canSubmit} onClick={() => void c.generate()}>↑</button>
    </div>
  </section>;
}
