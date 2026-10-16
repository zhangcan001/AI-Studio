import { useEffect, useMemo, useRef, useState, type ReactElement } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { StoryEntity, StoryEntityKind } from "../../types/storyEntity";
import { STORY_ENTITIES_CHANGED, entityAvatarAssetId } from "../../types/storyEntity";
import { CandidateThumbnail, loadLibraryImageUrl } from "./CreateMediaPreview";
import "./CreatePage.css";
import type { GeneratorOption } from "../../product/types";
import type { DraftValue } from "../../types/generation";
import { parseShotPrompt, resolvePreset, type ParsedShotPrompt, type ResolutionPresetLike } from "./shotPromptParser";
import { coverAsset, runPercent, shotDuration, shotResolution, storyboardStatusLabels, type StoryboardShot, type StoryboardShotStatus } from "./storyboardModel";

const KIND_LABEL: Record<StoryEntityKind, string> = { character: "角色", scene: "场景", prop: "物品" };
const KINDS: StoryEntityKind[] = ["scene", "character", "prop"];
const FILTERS: (StoryboardShotStatus | "ALL")[] = ["ALL", "TODO", "GENERATING", "REVIEW", "COMPLETED", "FAILED"];

interface RowsProps {
  projectId: string;
  items: StoryboardShot[];
  busy: boolean;
  onOpen: (shotId: string) => void;
  onCreate: () => void;
  onChanged: () => void;
  onQueue: () => void;
}

/**
 * Shot-row editing view. Every write uses an existing formal path:
 * name → creation.updateShot, delete → creation.deleteShot, entity binding →
 * storyEntities.bindShot, result → creation.selectResult, generation →
 * creation.readinessGet + creation.generate (Production Queue). Inline prompt,
 * duration and aspect/resolution persist through creation.updateShot (stage
 * prompt + generator scalar values validated by the selected recipe).
 */
export function StoryboardShotRows({ projectId, items, busy, onOpen, onCreate, onChanged, onQueue }: RowsProps) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("ALL");
  const [entities, setEntities] = useState<StoryEntity[]>([]);
  const [rev, setRev] = useState(0);
  useEffect(() => {
    const bump = () => setRev(v => v + 1);
    window.addEventListener(STORY_ENTITIES_CHANGED, bump);
    return () => window.removeEventListener(STORY_ENTITIES_CHANGED, bump);
  }, []);
  useEffect(() => {
    let alive = true;
    const list = productClient.storyEntities?.list;
    if (typeof list !== "function") return;
    void Promise.resolve(list(projectId, null)).then(next => { if (alive) setEntities(next ?? []); }).catch(() => { if (alive) setEntities([]); });
    return () => { alive = false; };
  }, [projectId, rev]);
  const urls = useEntityCovers(projectId, entities);
  const [generators, setGenerators] = useState<GeneratorOption[]>([]);
  useEffect(() => {
    let alive = true;
    const list = productClient.creation?.generatorsList;
    if (typeof list !== "function") return;
    void Promise.resolve(list(projectId, "video", null)).then(next => { if (alive) setGenerators(next ?? []); }).catch(() => { if (alive) setGenerators([]); });
    return () => { alive = false; };
  }, [projectId]);
  const q = query.trim().toLowerCase();
  const visible = items.filter(item => (filter === "ALL" || item.status === filter)
    && (!q || item.name.toLowerCase().includes(q) || (item.shot?.prompt ?? "").toLowerCase().includes(q)));
  return <section className="storyboard-shot-rows" aria-label="分镜编辑">
    <div className="storyboard-shot-toolbar">
      <input type="search" aria-label="搜索分镜" placeholder="搜索分镜" value={query} onChange={e => setQuery(e.target.value)} />
      <span className="storyboard-shot-count">{visible.length}/{items.length} 个分镜</span>
      <div role="group" aria-label="状态筛选" className="storyboard-shot-filters">
        {FILTERS.map(key => <button type="button" key={key} aria-pressed={filter === key} onClick={() => setFilter(key)}>{key === "ALL" ? "全部" : storyboardStatusLabels[key]}</button>)}
      </div>
      <button type="button" onClick={onQueue}>任务队列</button>
    </div>
    {visible.map(item => <ShotRowCard key={`${projectId}:${item.id}`} projectId={projectId} item={item} entities={entities} urls={urls} busy={busy}
      generator={rowGenerator(generators, item.shot?.selectionRef ?? null)}
      onOpen={() => onOpen(item.id)} onCreate={onCreate} onChanged={onChanged} />)}
    {!visible.length && <p className="storyboard-empty-note">没有符合条件的分镜。</p>}
  </section>;
}

/**
 * The shot's own generator; for a shot without a saved stage config, the same
 * default the create page picks (recommended+available, then available), so
 * duration/aspect can be set inline and the first save creates the stage config.
 */
export function rowGenerator(generators: GeneratorOption[], selectionRef: string | null) {
  if (selectionRef) return generators.find(g => g.selectionRef === selectionRef);
  return generators.find(g => g.recommended && g.availability) ?? generators.find(g => g.availability) ?? generators.find(g => g.recommended) ?? generators[0];
}

function useEntityCovers(projectId: string, entities: StoryEntity[]) {
  const [urls, setUrls] = useState<Record<string, string>>({});
  const key = entities.map(e => entityAvatarAssetId(e) ?? "").join("|");
  useEffect(() => {
    let alive = true;
    for (const id of new Set(entities.map(e => entityAvatarAssetId(e)).filter((v): v is string => Boolean(v)))) {
      if (urls[id]) continue;
      void loadLibraryImageUrl(projectId, id).then(u => { if (alive && u) setUrls(m => m[id] === u ? m : { ...m, [id]: u }); });
    }
    return () => { alive = false; };
  }, [projectId, key]);
  return urls;
}

function Avatar({ entity, url }: { entity: StoryEntity; url?: string }) {
  return url ? <img className="shot-row-avatar" src={url} alt="" /> : <span className="shot-row-avatar glyph" aria-hidden="true">{entity.name.slice(0, 1)}</span>;
}

function newKey() { try { return crypto.randomUUID(); } catch { return `sb-${Date.now()}-${Math.random().toString(36).slice(2)}`; } }

type Scalars = Record<string, DraftValue>;
const SCALAR_TYPES = new Set(["integer", "number", "seed_random", "seed_fixed"]);
/** Only generator scalars are persisted in stage configs; prompt/assets are owned elsewhere. */
function scalarValues(values: Record<string, DraftValue> | undefined): Scalars {
  return Object.fromEntries(Object.entries(values ?? {}).filter(([, v]) => SCALAR_TYPES.has(v.type)));
}
function numberOf(v: DraftValue | undefined) { return v?.type === "integer" || v?.type === "number" ? v.value : undefined; }
function aspectKey(p: ResolutionPresetLike) { const m = /-(\d+)x(\d+)$/.exec(p.id); return m ? `${m[1]}:${m[2]}` : `${p.width}:${p.height}`; }

interface Recognized { parsed: ParsedShotPrompt; values?: Scalars; addEntityIds: string[]; labels: string[] }

/** Turns parser output into concrete writes for this shot + generator; labels describe them. */
function recognize(text: string, entities: StoryEntity[], bound: StoryEntity[], generator: GeneratorOption | undefined, current: Scalars): Recognized {
  const parsed = parseShotPrompt(text, entities);
  const labels: string[] = [];
  let values: Scalars | undefined;
  const durationField = generator?.fields.find(f => f.key === "duration_seconds");
  if (parsed.durationSeconds !== undefined) {
    if (durationField && (durationField.type === "integer" || durationField.type === "number")) {
      const min = "min" in durationField && typeof durationField.min === "number" ? durationField.min : undefined;
      const max = "max" in durationField && typeof durationField.max === "number" ? durationField.max : undefined;
      const sec = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, parsed.durationSeconds));
      if (numberOf(current.duration_seconds) !== sec) { values = { ...(values ?? current), duration_seconds: { type: durationField.type as "integer", value: sec } }; labels.push(`时长 ${sec} 秒`); }
    } else labels.push(`时长 ${parsed.durationSeconds} 秒（当前生成器无此参数）`);
  }
  const w = numberOf(current.width), h = numberOf(current.height);
  const preset = resolvePreset(parsed, generator?.resolutionPresets ?? [], w && h ? { width: w, height: h } : undefined);
  if (preset && (preset.width !== w || preset.height !== h)) { values = { ...(values ?? current), width: { type: "integer", value: preset.width }, height: { type: "integer", value: preset.height } }; labels.push(`画幅 ${aspectKey(preset)} · ${preset.width}×${preset.height}`); }
  else if ((parsed.aspect || parsed.resolution) && !generator?.resolutionPresets?.length) labels.push("画幅/分辨率（请先在详情中选择生成器）");
  const addEntityIds = parsed.entityIds.filter(id => !bound.some(b => b.id === id));
  for (const id of addEntityIds) { const e = entities.find(x => x.id === id); if (e) labels.push(`${KIND_LABEL[e.kind]} @${e.name}`); }
  if (parsed.shotNumber !== undefined) labels.push(`镜号 ${parsed.shotNumber}（仅识别，顺序不变）`);
  return { parsed, values, addEntityIds, labels };
}

function ShotRowCard({ projectId, item, entities, urls, busy, generator, onOpen, onCreate, onChanged }: {
  projectId: string; item: StoryboardShot; entities: StoryEntity[]; urls: Record<string, string>; busy: boolean; generator?: GeneratorOption;
  onOpen: () => void; onCreate: () => void; onChanged: () => void;
}) {
  const [bound, setBound] = useState<StoryEntity[]>([]);
  const [prompt, setPrompt] = useState(item.shot?.prompt ?? "");
  const [name, setName] = useState(item.name);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const [note, setNote] = useState<string>();
  const scopeEpoch = useRef(0);
  const bindingEpoch = useRef(0);
  const [bindingsReady, setBindingsReady] = useState(false);
  useEffect(() => () => { ++scopeEpoch.current; }, [projectId, item.id]);
  useEffect(() => { setPrompt(item.shot?.prompt ?? ""); }, [item.shot?.prompt]);
  useEffect(() => { setName(item.name); }, [item.name]);
  useEffect(() => {
    const token = ++bindingEpoch.current;
    setBindingsReady(false);
    const listForShot = productClient.storyEntities?.listForShot;
    if (typeof listForShot === "function") {
      void Promise.resolve().then(() => listForShot(projectId, item.id)).then(next => {
        if (token !== bindingEpoch.current) return;
        setBound(next ?? []); setBindingsReady(true);
      }).catch(cause => { if (token === bindingEpoch.current) setError(normalizeProductError(cause).message); });
    }
    return () => { ++bindingEpoch.current; };
  }, [projectId, item.id, entities]);
  const disabled = busy || working;
  const bindingsDisabled = disabled || !bindingsReady;
  const run = async (fn: () => Promise<unknown>) => {
    const token = scopeEpoch.current;
    setWorking(true); setError(undefined); setNote(undefined);
    try { await fn(); } catch (cause) { if (token === scopeEpoch.current) setError(normalizeProductError(cause).message); } finally { if (token === scopeEpoch.current) setWorking(false); }
  };
  const replaceBindings = async (ids: string[]) => {
    const scope = scopeEpoch.current;
    ++bindingEpoch.current;
    const next = await productClient.storyEntities.bindShot(projectId, item.id, ids);
    if (scope !== scopeEpoch.current) return;
    ++bindingEpoch.current; // A read started before this write completed is stale too.
    setBound(next ?? []); setBindingsReady(true);
  };
  const toggle = (id: string) => {
    if (bindingsDisabled) return;
    void run(() => replaceBindings(bound.some(e => e.id === id) ? bound.filter(e => e.id !== id).map(e => e.id) : [...bound.map(e => e.id), id]));
  };
  const saveName = () => { const next = name.trim(); if (!next || next === item.name) { setName(item.name); return; } void run(async () => { await productClient.creation.updateShot(projectId, { shotId: item.id, name: next }); onChanged(); }); };
  const remove = () => { if (!window.confirm(`删除分镜「${item.name}」？此操作不可撤销。`)) return; void run(async () => { await productClient.creation.deleteShot(projectId, item.id); onChanged(); }); };
  const selectionRef = item.shot?.selectionRef ?? null;
  const current = useMemo(() => scalarValues(item.shot?.values), [item.shot?.values]);
  const [undo, setUndo] = useState<{ values?: Scalars; boundIds?: string[] }>();
  const save = (patch: { prompt?: string; values?: Scalars }) => productClient.creation.updateShot(projectId, { shotId: item.id, name: item.name, stage: "video", ...patch, ...(patch.values ? { selectionRef: selectionRef ?? generator?.selectionRef } : {}) });
  const preview = useMemo(() => recognize(prompt, entities, bound, generator, current), [prompt, entities, bound, generator, current]);
  /** Saves the prompt and applies recognized fields; the note lists exactly what was filled, with undo. */
  const savePrompt = () => {
    if (bindingsDisabled) return;
    const text = prompt.trim();
    const promptChanged = text !== (item.shot?.prompt ?? "").trim();
    const r = recognize(text, entities, bound, generator, current);
    if (!promptChanged && !r.values && !r.addEntityIds.length) return;
    void run(async () => {
      const before = { values: r.values ? current : undefined, boundIds: r.addEntityIds.length ? bound.map(e => e.id) : undefined };
      await save({ ...(promptChanged ? { prompt: text } : {}), ...(r.values && generator ? { values: r.values } : {}) });
      if (r.addEntityIds.length) await replaceBindings([...bound.map(e => e.id), ...r.addEntityIds]);
      const applied = r.labels.filter(l => !l.includes("（"));
      if (applied.length) { setUndo(before); setNote(`已识别并填入：${applied.join(" · ")}`); } else { setUndo(undefined); setNote("提示词已保存"); }
      onChanged();
    });
  };
  const undoRecognized = () => undo && !bindingsDisabled && void run(async () => {
    if (undo.values && generator) await save({ values: undo.values });
    if (undo.boundIds) await replaceBindings(undo.boundIds);
    setUndo(undefined); setNote("已撤销自动填入"); onChanged();
  });
  const saveValues = (next: Scalars, afterSave?: () => void) => void run(async () => { await save({ values: next }); afterSave?.(); onChanged(); });
  const durationField = generator?.fields.find(f => f.key === "duration_seconds" && (f.type === "integer" || f.type === "number"));
  const presets = generator?.resolutionPresets ?? [];
  const curW = numberOf(current.width), curH = numberOf(current.height);
  const currentPreset = presets.find(p => p.width === curW && p.height === curH);
  const aspects = [...new Set(presets.map(aspectKey))];
  const currentAspect = currentPreset ? aspectKey(currentPreset) : undefined;
  const sizeList = currentAspect ? presets.filter(p => aspectKey(p) === currentAspect) : presets;
  const choosePreset = (p?: ResolutionPresetLike) => p && saveValues({ ...current, width: { type: "integer", value: p.width }, height: { type: "integer", value: p.height } });
  const chooseAspect = (a: string) => choosePreset(resolvePreset({ aspect: a }, presets, curW && curH ? { width: curW, height: curH } : undefined));
  const savedDuration = numberOf(current.duration_seconds);
  const [duration, setDuration] = useState<string>(() => savedDuration === undefined ? "" : String(savedDuration));
  const durationDirty = useRef(false);
  const durationScope = `${projectId}:${item.id}:${selectionRef ?? ""}`;
  const previousDurationScope = useRef(durationScope);
  useEffect(() => {
    if (previousDurationScope.current !== durationScope) { durationDirty.current = false; previousDurationScope.current = durationScope; }
    if (!durationDirty.current) setDuration(savedDuration === undefined ? "" : String(savedDuration));
  }, [savedDuration, durationScope]);
  const saveDuration = () => {
    if (!durationField) return;
    const prev = numberOf(current.duration_seconds);
    if (duration === "" ) { if (prev !== undefined) { const { duration_seconds: _, ...rest } = current; saveValues(rest, () => { durationDirty.current = false; }); } else durationDirty.current = false; return; }
    const n = Number(duration); if (!Number.isFinite(n) || n === prev) { durationDirty.current = false; setDuration(prev === undefined ? "" : String(prev)); return; }
    const value = durationField.type === "integer" ? Math.round(n) : n;
    saveValues({ ...current, duration_seconds: { type: durationField.type as "integer", value } }, () => { durationDirty.current = false; setDuration(String(value)); });
  };
  const noGen = !generator ? "请先在详情中选择生成器" : undefined;
  const generationDraft = JSON.stringify([projectId, item.id, selectionRef, prompt, item.shot?.values]);
  const latestGenerationDraft = useRef(generationDraft);
  latestGenerationDraft.current = generationDraft;
  const generate = () => selectionRef && item.shot && void run(async () => {
    const token = scopeEpoch.current;
    const isCurrent = () => token === scopeEpoch.current && generationDraft === latestGenerationDraft.current;
    const values = { ...item.shot!.values, prompt: { type: "string" as const, value: prompt } };
    const base = { projectId, shotId: item.id, stage: "video" as const, selectionRef, values };
    const readiness = await productClient.creation.readinessGet({ ...base, submissionIdempotencyKey: "readiness-only" });
    if (!isCurrent()) return;
    if (!readiness.ready) { setError(readiness.issues.map(i => i.message).join("；") || "尚未就绪，请在详情中完成设置"); return; }
    const ack = await productClient.creation.generate({ ...base, submissionIdempotencyKey: newKey() });
    if (!isCurrent()) return; // Accepted Queue work remains owned by the backend.
    setNote(ack.accepted ? "已写入生产队列" : "未被接受");
    onChanged();
  });
  const cover = coverAsset(item);
  const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  const byKind = useMemo(() => Object.fromEntries(KINDS.map(k => [k, bound.filter(e => e.kind === k)])) as Record<StoryEntityKind, StoryEntity[]>, [bound]);
  const chips = (list: StoryEntity[]) => list.length ? list.map(e => <span key={e.id} className="shot-row-inline-chip"><Avatar entity={e} url={(entityAvatarAssetId(e) && urls[entityAvatarAssetId(e)!]) || undefined} />{e.name}</span>) : <span className="muted">未绑定</span>;
  const content = renderPrompt(prompt, entities, urls);
  return <article className="shot-row-card" aria-label={`分镜 ${item.ordinal + 1} ${item.name}`} data-status={item.status}>
    <div className="shot-row-index">{item.ordinal + 1}</div>
    <div className="shot-row-media">
      <div className="shot-row-preview">{cover ? <CandidateThumbnail asset={cover} playable /> : <span className="storyboard-cover-empty">暂无结果</span>}
        {percent !== null && <span className="storyboard-ring" style={{ ["--p" as string]: `${percent}` }} aria-label={`进度 ${percent}%`}>{percent}%</span>}</div>
      {item.candidates.length > 1 && <div className="shot-row-strip" aria-label="候选结果">{item.candidates.map(asset => <button type="button" key={asset.id} aria-pressed={item.shot?.selectedResultId === asset.id}
        title={`选为结果：${asset.name}`} disabled={disabled || item.shot?.selectedResultId === asset.id}
        onClick={() => void run(async () => { await productClient.creation.selectResult(projectId, item.id, "video", asset.id); onChanged(); })}><CandidateThumbnail asset={asset} playable={false} /></button>)}</div>}
      <small className="shot-row-aspect" title="画幅在详情中设置">{shotResolution(item.shot)}</small>
    </div>
    <div className="shot-row-actions">
      <button type="button" className="primary" disabled={disabled || !selectionRef} title={selectionRef ? "写入生产队列并生成" : "请先在详情中选择生成器"} onClick={generate}>生成视频</button>
      <ul className="shot-row-entities" aria-label="项目角色与场景" aria-busy={!bindingsReady}>{entities.length ? entities.map(e => { const aid = entityAvatarAssetId(e); const on = bound.some(b => b.id === e.id); return <li key={e.id}><button type="button" className={on ? "bound" : undefined} aria-pressed={on} disabled={bindingsDisabled} title={(on ? "解除绑定 " : "绑定 ") + `${KIND_LABEL[e.kind]} · ${e.name}`} onClick={() => toggle(e.id)}><Avatar entity={e} url={aid ? urls[aid] : undefined} /><small>{e.name}</small></button></li>; }) : <li className="muted">素材库中还没有角色与场景</li>}</ul>
    </div>
    <div className="shot-row-prompt">
      <textarea aria-label={`${item.name} 提示词`} placeholder="续写提示词...（可写 5秒 / 16:9 / 竖屏 / 720p / @角色名，自动识别）" value={prompt} disabled={bindingsDisabled}
        onChange={e => setPrompt(e.target.value)} onBlur={savePrompt} onKeyDown={e => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); savePrompt(); } }} title="失焦或 Ctrl+Enter 保存；时长、画幅、分辨率、@角色 会被自动识别并填入" />
      {preview.labels.length > 0 && <p className="shot-row-recognized" aria-label="识别结果">识别到：{preview.labels.join(" · ")}</p>}
      <dl className="shot-row-structured" aria-label="分镜描述">
        <div><dt>镜号</dt><dd>{item.ordinal + 1}</dd></div>
        <div><dt>时长</dt><dd>{shotDuration(item.shot)}</dd></div>
        <div><dt>画幅</dt><dd>{shotResolution(item.shot)}</dd></div>
        <div><dt>场景</dt><dd>{chips(byKind.scene)}</dd></div>
        <div><dt>角色</dt><dd>{chips(byKind.character)}</dd></div>
        <div><dt>物品</dt><dd>{chips(byKind.prop)}</dd></div>
        <div><dt>画面内容</dt><dd>{content}</dd></div>
      </dl>
      <div className="shot-row-toolbar"><button type="button" onClick={onOpen}>编辑</button><button type="button" onClick={onOpen}>详情</button></div>
      {error && <p role="alert" className="storyboard-error">{error}</p>}
      {note && <p role="status">{note}{undo && <button type="button" className="shot-row-undo" disabled={bindingsDisabled} onClick={undoRecognized}>撤销</button>}</p>}
    </div>
    <div className="shot-row-fields">
      <label><span>镜号</span><output>{item.ordinal + 1}</output></label>
      <label><span>名称</span><input aria-label={`${item.name} 名称`} value={name} disabled={disabled} onChange={e => setName(e.target.value)} onBlur={saveName} onKeyDown={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} /></label>
      {durationField
        ? <label><span>时长</span><input type="number" aria-label={`${item.name} 时长（秒）`} min={"min" in durationField ? durationField.min : undefined} max={"max" in durationField ? durationField.max : undefined} step={durationField.type === "integer" ? 1 : "any"} value={duration} disabled={disabled} onChange={e => { durationDirty.current = true; setDuration(e.target.value); }} onBlur={saveDuration} onKeyDown={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} /><small>秒</small></label>
        : <label><span>时长</span><output title={noGen ?? "当前生成器没有时长参数"}>{shotDuration(item.shot)}</output></label>}
      {presets.length > 0
        ? <><label><span>画幅</span><select aria-label={`${item.name} 画幅`} value={currentAspect ?? ""} disabled={disabled} onChange={e => chooseAspect(e.target.value)}>{!currentAspect && <option value="" disabled>未设置</option>}{aspects.map(a => <option key={a} value={a}>{a}</option>)}</select></label>
          <label><span>分辨率</span><select aria-label={`${item.name} 分辨率`} value={currentPreset?.id ?? ""} disabled={disabled} onChange={e => choosePreset(presets.find(p => p.id === e.target.value))}>{!currentPreset && <option value="" disabled>{curW && curH ? `${curW}×${curH}` : "未设置"}</option>}{sizeList.map(p => <option key={p.id} value={p.id}>{p.label || `${p.width}×${p.height}`}</option>)}</select></label></>
        : <label><span>画幅</span><output title={noGen ?? "当前生成器没有分辨率预设"}>{shotResolution(item.shot)}</output></label>}
      {KINDS.map(kind => <label key={kind}><span>{KIND_LABEL[kind]}</span><output>{byKind[kind].map(e => `@${e.name}`).join("、") || "—"}</output></label>)}
      <div className="shot-row-field-actions">
        <button type="button" className="success" disabled={disabled} title="新分镜追加到末尾" onClick={onCreate}>新建分镜</button>
        <button type="button" className="danger" disabled={disabled} onClick={remove}>删除分镜</button>
      </div>
      <span className="storyboard-badge">{item.error ? "读取失败" : storyboardStatusLabels[item.status]}</span>
    </div>
  </article>;
}

/** Highlights @EntityName tokens of real project entities as avatar chips. */
function renderPrompt(text: string, entities: StoryEntity[], urls: Record<string, string>) {
  if (!text.trim()) return <span className="muted">尚未填写提示词</span>;
  const names = [...entities].sort((a, b) => b.name.length - a.name.length);
  const out: (string | ReactElement)[] = [];
  let rest = text; let key = 0;
  while (rest) {
    const at = rest.indexOf("@");
    if (at < 0) { out.push(rest); break; }
    out.push(rest.slice(0, at));
    const hit = names.find(e => rest.startsWith(`@${e.name}`, at));
    if (hit) { out.push(<span key={key++} className="shot-row-inline-chip"><Avatar entity={hit} url={(entityAvatarAssetId(hit) && urls[entityAvatarAssetId(hit)!]) || undefined} />{hit.name}</span>); rest = rest.slice(at + 1 + hit.name.length); }
    else { out.push("@"); rest = rest.slice(at + 1); }
  }
  return <>{out}</>;
}
