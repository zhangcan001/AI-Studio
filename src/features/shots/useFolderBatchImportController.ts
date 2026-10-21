import { useEffect, useRef, useState } from "react";
import type { GeneratorOption } from "../../product/types";
import type { FolderImportLegacyDefaults } from "../../types/folderBatchImport";
import { productClient } from "../../product/client";
import { IpcError } from "../../services/ipc";
import { toUserMessage } from "../../i18n/errorMessages";
function importError(e:unknown):string {if(e instanceof IpcError && typeof e.details==="object" && e.details!==null && "code" in e.details && typeof e.details.code==="string")return e.details.code+"："+e.message;return toUserMessage(e);}
import type { FolderImportPreview, FolderImportResult } from "../../types/folderBatchImport";

/** Ephemeral import session only; never owns a Shot or Studio Store draft. */
export function useFolderBatchImportController(projectId: string, onImported: () => unknown) {
  const [uiProject,setUiProject]=useState(projectId);
  const [preview, setPreview] = useState<FolderImportPreview>();
  const [result, setResult] = useState<FolderImportResult>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState<"pick" | "rescan" | "commit">();
  const [suffix, setSuffix] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [createQueue, setCreateQueue] = useState(true);
  const [autoStart, setAutoStart] = useState(false);
  const [legacy,setLegacy]=useState<FolderImportLegacyDefaults>({selectionRef:"",durationSeconds:6,width:0,height:0});
  const [validatedLegacy,setValidatedLegacy]=useState("");
  const [generators,setGenerators]=useState<GeneratorOption[]>([]);
  const legacyRef=useRef(legacy);legacyRef.current=legacy;
  const scope = useRef(projectId); scope.current = projectId;
  const operation = useRef(0); const locked = useRef(false); const mounted = useRef(true);
  const session = useRef<{ project: string; id: string } | undefined>(undefined);
  const imported = useRef(onImported); imported.current = onImported;
  const discard = (id: string) => { void productClient.folderImport.discard(id).catch(() => { /* backend TTL owns abandoned previews */ }); };
  useEffect(() => {
    setUiProject(projectId);
    mounted.current = true; locked.current = false; operation.current++;
    setPreview(undefined); setResult(undefined); setError(undefined); setBusy(undefined);
    setSuffix(""); setSelected(new Set()); setAutoStart(false); setCreateQueue(true);setLegacy({selectionRef:"",durationSeconds:6,width:0,height:0});setValidatedLegacy("");setGenerators([]);
    return () => { mounted.current = false; operation.current++; if (session.current?.project === projectId) { discard(session.current.id); session.current = undefined; } };
  }, [projectId]);
  const ownsPreview=Boolean(preview && session.current?.project===projectId && uiProject===projectId);
  const legacyMode=Boolean(ownsPreview && preview?.formatSource!=="manifest");
  useEffect(()=>{let cancelled=false;if(!legacyMode)return;const project=projectId;void productClient.creation.generatorsList(project,"video").then(options=>{if(!cancelled && scope.current===project)setGenerators(options.filter(o=>o.mediaKind==="video"));}).catch(e=>{if(!cancelled && scope.current===project)setError(importError(e));});return()=>{cancelled=true;};},[projectId,legacyMode]);
  const current = (project: string, ticket: number) => mounted.current && scope.current === project && operation.current === ticket;
  const accept = (p: FolderImportPreview) => {
    session.current = { project: projectId, id: p.sessionId }; setPreview(p); setResult(undefined);
    setSelected(new Set(p.rows.filter(r => r.errors.length === 0).map(r => r.rowKey)));
    setCreateQueue(p.createQueueDefault); setAutoStart(false); setSuffix(p.nameSuffix ?? "");setValidatedLegacy(p.formatSource==="manifest"?"":JSON.stringify(legacyRef.current));
  };
  const begin = (kind: "pick" | "rescan" | "commit") => {
    if (locked.current) return;
    locked.current = true; const ticket = ++operation.current; setBusy(kind); setError(undefined);
    return ticket;
  };
  const finish = (project: string, ticket: number) => { if (current(project, ticket)) { locked.current = false; setBusy(undefined); } };
  const pick = async () => {
    const project = projectId, ticket = begin("pick"); if (ticket === undefined) return;
    try {
      const p = await productClient.folderImport.pick(project);
      if (!current(project, ticket)) { if (p) discard(p.sessionId); return; }
      if (p) { if (session.current) discard(session.current.id); accept(p); }
    } catch (e) { if (current(project, ticket)) setError(importError(e)); }
    finally { finish(project, ticket); }
  };
  const rescan = async () => {
    if (!ownsPreview || !preview || result) return;
    const project = projectId, ticket = begin("rescan"); if (ticket === undefined) return;
    try { const p = await productClient.folderImport.rescan(preview.sessionId, { nameSuffix: suffix || null, ...(legacyMode ? {legacy} : {}) }); if (current(project, ticket)) accept(p); }
    catch (e) { if (current(project, ticket)) setError(importError(e)); }
    finally { finish(project, ticket); }
  };
  const dirty = Boolean(preview && ((preview.nameSuffix ?? "") !== suffix || (legacyMode && JSON.stringify(legacy)!==validatedLegacy)));
  const overQueueLimit = Boolean(preview && createQueue && selected.size > preview.maxQueueItems);
  const canCommit = Boolean(ownsPreview && preview && !busy && !result && !dirty && selected.size && !overQueueLimit && preview.errors.length === 0);
  const commit = async () => {
    if (!canCommit || !preview) return;
    const project = projectId, ticket = begin("commit"); if (ticket === undefined) return;
    try {
      const r = await productClient.folderImport.commit({ sessionId: preview.sessionId, previewVersion: preview.previewVersion, manifestSha256: preview.manifestSha256, includeRowKeys: preview.rows.filter(row => selected.has(row.rowKey) && !row.errors.length).map(row => row.rowKey), createQueue, autoStart: createQueue && autoStart, nameSuffix: preview.nameSuffix });
      if (!current(project, ticket)) return;
      setResult(r);
      // Refresh failures must not turn a completed write into a retryable commit.
      try { await imported.current(); } catch { if (current(project, ticket)) setError("导入结果已保存，但列表刷新失败，请手动刷新；不要重复提交。"); }
    } catch (e) { if (current(project, ticket)) setError(importError(e)); }
    finally { finish(project, ticket); }
  };
  const toggle = (key: string) => { if (busy || result || !preview?.rows.some(r => r.rowKey === key && !r.errors.length)) return; setSelected(old => { const next = new Set(old); if (next.has(key)) next.delete(key); else next.add(key); return next; }); };
  return { legacyMode,legacy,setLegacy,generators,preview:ownsPreview?preview:undefined, result:ownsPreview?result:undefined, error:uiProject===projectId?error:undefined, busy:uiProject===projectId?busy:undefined, suffix, setSuffix, selected, toggle, createQueue, setCreateQueue: (value: boolean) => { setCreateQueue(value); if (!value) setAutoStart(false); }, autoStart, setAutoStart, dirty, overQueueLimit, canCommit, pick, rescan, commit };
}
