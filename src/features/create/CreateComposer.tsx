import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RecipeField } from "../../types/generation";
import type { CreateController } from "./CreateController";
import { assetIds, fieldLabel, mediaKind, mediaValue } from "./createModel";
import { modeLabel } from "../../product/generatorPresentation";
import { AssetImageThumb, CandidateThumbnail, loadLibraryImageUrl, useMediaLightbox } from "./CreateMediaPreview";
import { ParamField, PromptPickerDialog, ReferencePanel, ResolutionPicker } from "./CreateInputs";
import { ReadinessStatus } from "./CreateResults";
import { useStudioStore } from "../../stores/studioStore";

type Kind = "image" | "video" | "audio";
type Asset = { id: string; name: string; mediaKind: string; thumbnailBytes?: number[] | null };
/** One bound media input. Images are numbered "Image N" in submit order (field order, then item order). */
export interface ComposerReference { n: number; label: string; field: RecipeField; fieldKey: string; index: number; kind: Kind; assetId: string; asset?: Asset }
const TOKEN = /Image (\d+)/g;
const AUTOSAVE_MS = 250;
const KIND_PREFIX: Record<Kind, string> = { image: "Image", video: "Video", audio: "Audio" };
/** Resolution presets come from backend h3_resolution.rs (megapixel ladder, multiple of 32). */

export function composerReferences(fields: RecipeField[], values: CreateController["values"], choices: Asset[]): ComposerReference[] {
  const out: ComposerReference[] = []; const counters: Record<Kind, number> = { image: 0, video: 0, audio: 0 };
  for (const field of fields) {
    const kind = mediaKind(field) as Kind | null; if (!kind) continue;
    assetIds(values[field.key]).forEach((id, index) => { const n = ++counters[kind]; out.push({ n, label: `${KIND_PREFIX[kind]} ${n}`, field, fieldKey: field.key, index, kind, assetId: id, asset: choices.find(a => a.id === id) }); });
  }
  return out;
}
function gcd(a: number, b: number): number { return b ? gcd(b, a % b) : a; }
export function aspectOf(w: number, h: number) { const g = gcd(w, h) || 1; const r = `${w / g}:${h / g}`; const known = ["16:9", "9:16", "1:1", "4:3", "3:4", "21:9"]; if (known.includes(r)) return r; const q = w / h; return known.find(k => { const [a, b] = k.split(":").map(Number); return Math.abs(a / b - q) < 0.03; }) ?? r; }
/** Image thumbs use loadLibraryImageUrl (library imageGet). */
function chipNode(ref: ComposerReference, url?: string) {
  const chip = document.createElement("span");
  chip.className = "composer-chip"; chip.contentEditable = "false"; chip.dataset.ref = String(ref.n);
  if (url) { const img = document.createElement("img"); img.src = url; img.alt = ""; chip.appendChild(img); }
  const label = document.createElement("span"); label.textContent = ref.label; chip.appendChild(label);
  return chip;
}
function build(root: HTMLElement, value: string, images: ComposerReference[], url: (r: ComposerReference) => string | undefined) {
  root.replaceChildren(); let last = 0;
  for (const m of value.matchAll(TOKEN)) {
    const ref = images.find(r => r.n === Number(m[1])); if (!ref) continue;
    if (m.index! > last) root.appendChild(document.createTextNode(value.slice(last, m.index)));
    root.appendChild(chipNode(ref, url(ref))); last = m.index! + m[0].length;
  }
  if (last < value.length) root.appendChild(document.createTextNode(value.slice(last)));
}
export function serializePrompt(node: Node): string {
  let out = "";
  node.childNodes.forEach(child => {
    if (child.nodeType === Node.TEXT_NODE) out += child.textContent ?? "";
    else if (child instanceof HTMLElement) {
      if (child.dataset.ref) out += `Image ${child.dataset.ref}`;
      else if (child.tagName === "BR") out += "\n";
      else { if (child.tagName === "DIV" && out && !out.endsWith("\n")) out += "\n"; out += serializePrompt(child); }
    }
  });
  return out;
}
function capacity(field: RecipeField, ids: string[]) { return ["images", "videos", "audios"].includes(field.type) ? ids.length < ("maxItems" in field && field.maxItems ? field.maxItems : Infinity) : !ids.length; }

/** The create page's single input surface: media inputs, prompt with Image N chips, parameters, readiness and submit.
 *  It edits the existing draft, persists media through the existing OCC save, and submits through the existing generate path. */
export function CreateComposer({ controller: c }: { controller: CreateController }) {
  const vi = c.videoInputs; const projectId = c.context?.projectId;
  const lightbox = useMediaLightbox();
  const fields = c.generator?.fields ?? [];
  const promptField = fields.find(f => f.type === "textarea" && f.key === "prompt");
  const negativeField = fields.find(f => f.type === "textarea" && f.key === "negative_prompt");
  const mediaFields = fields.filter(f => mediaKind(f));
  const allChoices: Asset[] = (vi.enabled ? vi.view?.assets : c.context?.mediaInputs) ?? [];
  const refs = composerReferences(fields, c.values, allChoices);
  const images = refs.filter(r => r.kind === "image");
  const [imgUrls, setImgUrls] = useState<Record<string, string>>({});
  const url = (r: ComposerReference) => r.kind === "image" ? imgUrls[r.assetId] : undefined;
  const prompt = c.values.prompt?.type === "string" ? c.values.prompt.value : "";
  const editor = useRef<HTMLDivElement>(null);
  const emitted = useRef<string | null>(null);
  const caret = useRef<Range | null>(null);
  const [picker, setPicker] = useState<"insert" | "add" | null>(null);
  const [promptPicker, setPromptPicker] = useState(false);
  const locked = c.busy || vi.busy || vi.loading || (vi.enabled && !vi.view);
  const targets = mediaFields.filter(f => capacity(f, assetIds(c.values[f.key])));
  const [targetKey, setTargetKey] = useState<string>();
  const target = targets.find(f => f.key === targetKey) ?? targets[0];
  const imageKey = images.map(r => `${r.n}:${r.assetId}:${imgUrls[r.assetId] ? 1 : 0}`).join("|") + `@${projectId ?? ""}`;
  const imageIds = images.map(r => r.assetId).join("|");
  useEffect(() => {
    if (!projectId) return; let alive = true;
    for (const id of new Set(imageIds.split("|").filter(Boolean))) void loadLibraryImageUrl(projectId, id).then(u => { if (alive && u) setImgUrls(m => m[id] === u ? m : { ...m, [id]: u }); });
    return () => { alive = false; };
  }, [projectId, imageIds]);

  // Media edits persist through the existing OCC save (no separate "save inputs" step). A failed save is not retried until the draft changes.
  const inputsKey = JSON.stringify(mediaFields.map(f => [f.key, assetIds(c.values[f.key])]));
  const failedKey = useRef<string | null>(null);
  // Only edits made here autosave; a restored (e.g. Library/Settings return) unsaved draft waits for an explicit save.
  const pendingSave = useRef(false);
  const saveRef = useRef(vi.save); saveRef.current = vi.save;
  useEffect(() => {
    if (!pendingSave.current || !vi.enabled || !vi.dirty || !vi.view || vi.loading || vi.busy) return;
    if (vi.error && failedKey.current === inputsKey) return;
    // Short debounce so consecutive slot edits land in one OCC save.
    const timer = setTimeout(() => { failedKey.current = inputsKey; pendingSave.current = false; void saveRef.current(); }, AUTOSAVE_MS);
    return () => clearTimeout(timer);
  }, [inputsKey, vi.enabled, vi.dirty, vi.view, vi.loading, vi.busy, vi.error]);
  const inputsState = !vi.enabled ? "draft" : vi.loading || !vi.view ? "loading" : vi.busy ? "saving" : vi.error ? "error" : vi.dirty ? "dirty" : "saved";

  useLayoutEffect(() => {
    const root = editor.current; if (!root) return;
    if (emitted.current === prompt && root.dataset.refs === imageKey) return;
    build(root, prompt, images, url); root.dataset.refs = imageKey; emitted.current = prompt;
  }, [prompt, imageKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const emit = () => { const root = editor.current; if (!root || !promptField) return; const next = serializePrompt(root); emitted.current = next; c.setValue("prompt", { type: "string", value: next }); };
  const saveCaret = () => { const sel = window.getSelection(); if (sel?.rangeCount && editor.current?.contains(sel.anchorNode)) caret.current = sel.getRangeAt(0).cloneRange(); };
  const insert = (ref: ComposerReference) => {
    const root = editor.current; if (!root || !promptField) return;
    const chip = chipNode(ref, url(ref)); const space = document.createTextNode(" ");
    const range = caret.current && root.contains(caret.current.startContainer) ? caret.current : null;
    if (range) { range.deleteContents(); range.insertNode(space); range.insertNode(chip); } else root.append(chip, space);
    root.focus(); // focus first: focusing may reset the selection, the caret must land after the new chip
    const sel = window.getSelection(); const r = document.createRange(); r.setStartAfter(space); r.collapse(true); if (sel) { sel.removeAllRanges(); sel.addRange(r); } caret.current = r.cloneRange();
    setPicker(null); emit();
  };
  const setField = (field: RecipeField, ids: string[]) => { pendingSave.current = true; if (ids.length) c.setValue(field.key, mediaValue(field, ids)); else c.removeValue(field.key); };
  const plural = (f: RecipeField) => ["images", "videos", "audios"].includes(f.type);
  const add = (field: RecipeField, id: string) => { const ids = assetIds(c.values[field.key]); setField(field, plural(field) ? [...ids.filter(x => x !== id), id] : [id]); };
  const importInto = async (field: RecipeField, folder: boolean) => {
    setPicker(null);
    const imported = await vi.importAssets(folder);
    const kind = mediaKind(field); const ids = assetIds(useStudioStore.getState().values[field.key]);
    const fresh = (imported ?? []).filter(a => a.mediaKind === kind).map(a => a.id).filter(id => !ids.includes(id));
    if (!fresh.length) return;
    const max = plural(field) && "maxItems" in field && field.maxItems ? field.maxItems : plural(field) ? Infinity : 1;
    setField(field, plural(field) ? [...ids, ...fresh].slice(0, max) : [fresh[0]]);
  };

  const currentMode = c.generator ? modeLabel(c.generator) : undefined;
  const modes = [...new Set(c.generators.filter(g => g.mediaKind === "video").map(g => modeLabel(g)))];
  const pickMode = (label: string) => {
    const options = c.generators.filter(g => g.mediaKind === "video" && modeLabel(g) === label);
    const next = options.find(g => g.selectionRef === c.selection) ?? options.find(g => g.availability && g.recommended) ?? options.find(g => g.availability);
    if (next && next.selectionRef !== c.selection) c.chooseGenerator(next.selectionRef);
  };
  const presets = c.generator?.resolutionPresets ?? [];
  const pickerPresets = presets;
  const w = c.values.width, h = c.values.height;
  const current = presets.find(p => w?.type === "integer" && h?.type === "integer" && p.width === w.value && p.height === h.value);
  const aspects = [...new Set(pickerPresets.map(p => aspectOf(p.width, p.height)))];
  const aspect = current ? aspectOf(current.width, current.height) : undefined;
  const chooseAspect = (a: string) => {
    const curArea = current ? current.width * current.height : 1344 * 768;
    const candidates = pickerPresets.filter(p => aspectOf(p.width, p.height) === a);
    const best = [...candidates].sort((x, y) => Math.abs(x.width * x.height - curArea) - Math.abs(y.width * y.height - curArea))[0];
    if (best) c.chooseResolution(best.id);
  };
  const durationField = fields.find(f => f.key === "duration_seconds" && (f.type === "integer" || f.type === "number"));
  const advancedFields = fields.filter(f => ["integer", "number", "seed"].includes(f.type) && f.key !== "duration_seconds" && !(presets.length && ["width", "height"].includes(f.key)));
  const canSubmit = !c.busy && Boolean(c.generator?.availability) && vi.ready && c.readinessState === "READY";

  if (!c.generator) return null;
  return <section className="create-composer" aria-label="创作输入框" data-inputs={inputsState}>
    <ReadinessStatus controller={c} />
    {vi.enabled && !mediaFields.length && <small className="composer-hint">T2V 无需图片输入。</small>}
    {vi.enabled && vi.dirty && !vi.busy && !pendingSave.current && <p role="status" className="composer-unsaved">输入尚未保存。<button type="button" onClick={() => void vi.save()}>保存输入</button></p>}
    {mediaFields.length > 0 && <div className="composer-refs" aria-label="参考素材">
      {mediaFields.map(field => {
        const ids = assetIds(c.values[field.key]); const kind = mediaKind(field) as Kind; const label = fieldLabel(field);
        const choices = allChoices.filter(a => a.mediaKind === kind && (!plural(field) || !ids.includes(a.id)));
        const required = "required" in field && field.required;
        return <div key={field.key} className="composer-slot" aria-label={`${label}输入`}>
          {refs.filter(r => r.fieldKey === field.key).map(ref => <div key={ref.assetId} className="composer-ref" data-kind={ref.kind}>
            <button type="button" className="composer-ref-thumb" title={`${ref.kind === "image" ? `插入 ${ref.label}（Alt+点击放大）` : ref.label} · ${ref.asset?.name ?? "素材不可用"}`} aria-label={ref.kind === "image" ? `插入 ${ref.label}` : `${ref.label} ${ref.asset?.name ?? ""}`} disabled={ref.kind !== "image" || !promptField} onMouseDown={e => e.preventDefault()} onClick={e => { if ((e.altKey || e.metaKey) && projectId && ref.kind === "image") lightbox?.open({ kind: "image", projectId, assetId: ref.assetId, name: ref.asset?.name ?? ref.label, url: url(ref) }); else insert(ref); }}>
              {ref.kind === "image" && url(ref) ? <img src={url(ref)} alt={`${ref.asset?.name ?? ref.label}缩略图`} /> : ref.kind === "video" && ref.asset?.thumbnailBytes?.length ? <CandidateThumbnail asset={{ ...ref.asset, mediaKind: "video", selected: false }} projectId={projectId} /> : <span className="composer-ref-glyph">{ref.kind === "audio" ? "♪" : ref.kind === "video" ? "▶" : ref.n}</span>}
              <small>{ref.label}</small></button>
            <button type="button" className="composer-ref-remove" aria-label={`${label}移除第${ref.index + 1}项`} disabled={locked} onClick={() => setField(field, ids.filter(id => id !== ref.assetId))}>×</button>
            {plural(field) && ids.length > 1 && <span className="composer-ref-order">
              <button type="button" aria-label={`${label}上移第${ref.index + 1}项`} disabled={locked || ref.index === 0} onClick={() => { const next = [...ids]; [next[ref.index - 1], next[ref.index]] = [next[ref.index], next[ref.index - 1]]; setField(field, next); }}>‹</button>
              <button type="button" aria-label={`${label}下移第${ref.index + 1}项`} disabled={locked || ref.index === ids.length - 1} onClick={() => { const next = [...ids]; [next[ref.index], next[ref.index + 1]] = [next[ref.index + 1], next[ref.index]]; setField(field, next); }}>›</button></span>}
            {!ref.asset && <small role="status" className="composer-ref-missing">素材不可用，请重新选择</small>}
          </div>)}
          <label className="composer-slot-add" title={`选择${label}`}><span>{plural(field) || !ids.length ? "+" : "⇄"} {label}{required ? "*" : ""}</span>
            <select id={`create-field-${field.key}`} aria-label={label} value={plural(field) ? "" : ids[0] ?? ""} disabled={locked || (plural(field) && !capacity(field, ids))} onChange={e => { const id = e.target.value; if (plural(field)) { if (id) add(field, id); } else setField(field, id ? [id] : []); }}>
              <option value="">{plural(field) ? "添加素材" : "未选择"}</option>{choices.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
            </select></label>
        </div>;
      })}
    </div>}
    {promptField && <div className="composer-prompt">
      <div ref={editor} id="create-field-prompt" className="composer-editor" contentEditable={!c.busy} suppressContentEditableWarning tabIndex={0} role="textbox" aria-multiline="true" aria-label={fieldLabel(promptField)} data-placeholder={images.length ? "描述画面；输入 @ 或点击上方缩略图插入 Image 标签" : "描述画面"}
        onInput={emit} onKeyUp={saveCaret} onMouseUp={saveCaret} onBlur={saveCaret}
        onKeyDown={e => { if (e.key === "@" && images.length) { e.preventDefault(); saveCaret(); setPicker("insert"); } else if (e.key === "Escape") setPicker(null); }}
        onPaste={e => { e.preventDefault(); const text = e.clipboardData.getData("text/plain"); const sel = window.getSelection(); if (sel?.rangeCount) { const r = sel.getRangeAt(0); r.deleteContents(); const node = document.createTextNode(text); r.insertNode(node); r.setStartAfter(node); r.collapse(true); sel.removeAllRanges(); sel.addRange(r); } emit(); }} />
      <span className="composer-count" aria-hidden="true">{prompt.length} 字</span>
    </div>}
    {picker === "insert" && <div className="composer-picker" role="listbox" aria-label="插入参考图">
      {images.map(ref => <button type="button" role="option" aria-selected="false" key={ref.n} onMouseDown={e => e.preventDefault()} onClick={() => insert(ref)} className="composer-picker-option">{url(ref) ? <img src={url(ref)} alt="" /> : <span className="composer-picker-glyph">{ref.n}</span>}<span>{ref.label}{ref.asset ? ` · ${ref.asset.name}` : ""}</span></button>)}
      <button type="button" className="composer-picker-close" onClick={() => setPicker(null)}>关闭</button></div>}
    {picker === "add" && target && <div className="composer-picker" role="dialog" aria-label="添加参考素材">
      {targets.length > 1 && <label className="composer-picker-target">添加到<select aria-label="添加到输入" value={target.key} onChange={e => setTargetKey(e.target.value)}>{targets.map(f => <option key={f.key} value={f.key}>{fieldLabel(f)}</option>)}</select></label>}
      {allChoices.filter(a => a.mediaKind === mediaKind(target) && !assetIds(c.values[target.key]).includes(a.id)).map(a => <button type="button" key={a.id} className="composer-picker-option" onClick={() => { add(target, a.id); setPicker(null); }}>{a.mediaKind === "image" && projectId ? <span aria-hidden="true"><AssetImageThumb projectId={projectId} assetId={a.id} name={a.name} size="md" interactive={false} /></span> : null}<span>{a.name}</span></button>)}
      {vi.enabled && <button type="button" onClick={() => void importInto(target, false)}>从电脑导入…</button>}
      {vi.enabled && mediaKind(target) === "image" && <button type="button" onClick={() => void importInto(target, true)}>导入图片文件夹…</button>}
      <button type="button" onClick={() => { setPicker(null); c.openLibrary(mediaKind(target) === "image" ? "images" : mediaKind(target) === "video" ? "videos" : "audio"); }}>在资源库查找</button>
      <button type="button" className="composer-picker-close" onClick={() => setPicker(null)}>关闭</button></div>}
    <div className="composer-toolbar">
      <button type="button" className="composer-icon" aria-label="添加参考素材" title={target ? `添加到${fieldLabel(target)}` : mediaFields.length ? "输入已满" : "当前模式不需要参考素材"} disabled={!target || locked} onClick={() => setPicker(picker === "add" ? null : "add")}>+</button>
      <div className="composer-segment" role="group" aria-label="生成类型">
        <button type="button" aria-pressed="true">视频</button>
        <select aria-label="视频模式" value={currentMode ?? ""} disabled={c.busy || vi.busy} onChange={e => pickMode(e.target.value)}>
          {!modes.includes(currentMode ?? "") && <option value={currentMode ?? ""}>{currentMode ?? "选择模式"}</option>}
          {modes.map(m => <option key={m} value={m} disabled={!c.generators.some(g => g.mediaKind === "video" && g.availability && modeLabel(g) === m)}>{m}</option>)}
        </select>
      </div>
      {promptField && <button type="button" className="composer-text" onClick={() => setPromptPicker(true)}>选择提示词</button>}
      <span className="composer-spacer" />
      {presets.length > 0 && <ResolutionPicker controller={c} presets={pickerPresets} current={current} />}
      {durationField && <label className="composer-pick" title="时长">◷<input id="create-field-duration_seconds" aria-label={fieldLabel(durationField)} type="number" min={"min" in durationField ? durationField.min : undefined} max={"max" in durationField ? durationField.max : undefined} step={("step" in durationField && durationField.step) || (durationField.type === "integer" ? 1 : "any")} disabled={c.busy} value={(() => { const v = c.values.duration_seconds; return v?.type === "integer" || v?.type === "number" ? v.value : ""; })()} onChange={e => e.target.value === "" ? c.removeValue("duration_seconds") : c.setValue("duration_seconds", { type: durationField.type as "integer", value: Number(e.target.value) })} />s</label>}
      {aspects.length > 0 && <label className="composer-pick" title="画面比例">▭<select aria-label="画面比例" value={aspect ?? ""} disabled={c.busy} onChange={e => chooseAspect(e.target.value)}>{!aspect && <option value="" disabled>比例</option>}{aspects.map(a => <option key={a} value={a}>{a}</option>)}</select></label>}
      <button type="button" className="composer-submit" aria-label={c.busy ? "正在提交…" : "生成"} title={canSubmit ? "写入生产队列并生成" : "请先完成准备检查"} disabled={!canSubmit} onClick={() => void c.generate()}>↑</button>
    </div>
    <details className="composer-advanced"><summary>高级设置</summary>
      {negativeField && <label>{fieldLabel(negativeField)}<textarea id="create-field-negative_prompt" aria-label={fieldLabel(negativeField)} rows={2} value={c.values.negative_prompt?.type === "string" ? c.values.negative_prompt.value : ""} onChange={e => c.setValue("negative_prompt", { type: "string", value: e.target.value })} /></label>}
      {advancedFields.length > 0 && <div className="create-parameters">{advancedFields.map(field => <ParamField key={field.key} field={field} controller={c} />)}</div>}
      {promptField && <button type="button" onClick={() => c.openLibrary("prompts")}>在资源库查找更多提示词</button>}
      {vi.enabled && <button type="button" disabled={c.busy || vi.busy} onClick={() => void vi.refresh()}>刷新已保存输入</button>}
      {!vi.enabled && mediaFields.length > 0 && <small>下拉列表显示当前项目近期100项素材，并包含当前镜头明确关联的素材。更早的素材可从资源库查找；这些媒体输入只属于本次草稿，不会修改镜头长期参考图。</small>}
      {!vi.enabled && c.context && <ReferencePanel context={c.context} save={c.setReferences} busy={c.busy} />}
      {c.generator.mode?.toUpperCase() === "REF2VA" && <small>混合参考：图片最多9项、视频3项、音频3项，共12项。可仅用合法视频；纯音频不支持。</small>}
      <small>媒体输入修改后自动按当前 Recipe 保存；单镜「生成」写入生产队列并尝试启动，批量镜头请在生产队列点「开始生产」。</small>
    </details>
    {promptPicker && <PromptPickerDialog controller={c} onClose={() => setPromptPicker(false)} />}
  </section>;
}
