import { useCallback, useEffect, useMemo, useState, type ReactElement } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { StoryEntity, StoryEntityKind } from "../../types/storyEntity";
import { STORY_ENTITIES_CHANGED, entityAvatarAssetId } from "../../types/storyEntity";
import { CandidateThumbnail, loadLibraryImageUrl } from "./CreateMediaPreview";
import "./CreatePage.css";
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
 * creation.readinessGet + creation.generate (Production Queue).
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
    {visible.map(item => <ShotRowCard key={item.id} projectId={projectId} item={item} entities={entities} urls={urls} busy={busy}
      onOpen={() => onOpen(item.id)} onCreate={onCreate} onChanged={onChanged} />)}
    {!visible.length && <p className="storyboard-empty-note">没有符合条件的分镜。</p>}
  </section>;
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

function ShotRowCard({ projectId, item, entities, urls, busy, onOpen, onCreate, onChanged }: {
  projectId: string; item: StoryboardShot; entities: StoryEntity[]; urls: Record<string, string>; busy: boolean;
  onOpen: () => void; onCreate: () => void; onChanged: () => void;
}) {
  const [bound, setBound] = useState<StoryEntity[]>([]);
  const [prompt, setPrompt] = useState(item.shot?.prompt ?? "");
  const [name, setName] = useState(item.name);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const [note, setNote] = useState<string>();
  useEffect(() => { setPrompt(item.shot?.prompt ?? ""); }, [item.shot?.prompt]);
  useEffect(() => { setName(item.name); }, [item.name]);
  const reloadBound = useCallback(() => {
    const listForShot = productClient.storyEntities?.listForShot;
    if (typeof listForShot !== "function") return;
    void Promise.resolve(listForShot(projectId, item.id)).then(next => setBound(next ?? [])).catch(() => setBound([]));
  }, [projectId, item.id]);
  useEffect(() => { reloadBound(); }, [reloadBound, entities]);
  const disabled = busy || working;
  const run = async (fn: () => Promise<unknown>) => {
    setWorking(true); setError(undefined); setNote(undefined);
    try { await fn(); } catch (cause) { setError(normalizeProductError(cause).message); } finally { setWorking(false); }
  };
  const bind = (ids: string[]) => run(async () => { setBound((await productClient.storyEntities.bindShot(projectId, item.id, ids)) ?? []); });
  const toggle = (id: string) => void bind(bound.some(e => e.id === id) ? bound.filter(e => e.id !== id).map(e => e.id) : [...bound.map(e => e.id), id]);
  const saveName = () => { const next = name.trim(); if (!next || next === item.name) { setName(item.name); return; } void run(async () => { await productClient.creation.updateShot(projectId, { shotId: item.id, name: next }); onChanged(); }); };
  const remove = () => { if (!window.confirm(`删除分镜「${item.name}」？此操作不可撤销。`)) return; void run(async () => { await productClient.creation.deleteShot(projectId, item.id); onChanged(); }); };
  const selectionRef = item.shot?.selectionRef ?? null;
  const generate = () => selectionRef && item.shot && void run(async () => {
    const values = { ...item.shot!.values, prompt: { type: "string" as const, value: prompt } };
    const base = { projectId, shotId: item.id, stage: "video" as const, selectionRef, values };
    const readiness = await productClient.creation.readinessGet({ ...base, submissionIdempotencyKey: "readiness-only" });
    if (!readiness.ready) { setError(readiness.issues.map(i => i.message).join("；") || "尚未就绪，请在详情中完成设置"); return; }
    const ack = await productClient.creation.generate({ ...base, submissionIdempotencyKey: newKey() });
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
      <ul className="shot-row-entities" aria-label="项目角色与场景">{entities.length ? entities.map(e => { const aid = entityAvatarAssetId(e); const on = bound.some(b => b.id === e.id); return <li key={e.id}><button type="button" className={on ? "bound" : undefined} aria-pressed={on} disabled={disabled} title={(on ? "解除绑定 " : "绑定 ") + `${KIND_LABEL[e.kind]} · ${e.name}`} onClick={() => toggle(e.id)}><Avatar entity={e} url={aid ? urls[aid] : undefined} /><small>{e.name}</small></button></li>; }) : <li className="muted">素材库中还没有角色与场景</li>}</ul>
    </div>
    <div className="shot-row-prompt">
      <textarea aria-label={`${item.name} 提示词`} placeholder="续写提示词..." value={prompt} disabled={disabled} onChange={e => setPrompt(e.target.value)} title="提示词随「生成视频」一起提交" />
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
      {note && <p role="status">{note}</p>}
    </div>
    <div className="shot-row-fields">
      <label><span>镜号</span><output>{item.ordinal + 1}</output></label>
      <label><span>名称</span><input aria-label={`${item.name} 名称`} value={name} disabled={disabled} onChange={e => setName(e.target.value)} onBlur={saveName} onKeyDown={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} /></label>
      <label><span>时长</span><output title="在详情中修改">{shotDuration(item.shot)}</output></label>
      <label><span>画幅</span><output title="在详情中修改">{shotResolution(item.shot)}</output></label>
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
