import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { LibraryCursor, MediaIntegrityReport, ResourceRef } from "../../product/libraryTypes";

export const SCAN_PAGE_SIZE = 20;
export const SCAN_RESULT_LIMIT = 50;
export interface MediaCheckResult { assetId: string; title: string; report?: MediaIntegrityReport; error?: string }
interface ScanState { projectId: string; checked: number; issues: number; incomplete: number; current?: string; results: MediaCheckResult[]; phase: "IDLE" | "RUNNING" | "STOPPING" | "STOPPED" | "COMPLETED"; error?: string }
export function mediaCheckSummary(r: MediaIntegrityReport) {
  const issues = [r.boundary === "REJECTED" && "安全边界拒绝", r.existence === "MISSING" && "文件缺失", r.readability === "UNREADABLE" && "不可读取", r.checksum === "MISMATCH" && "校验不一致", r.checksum === "INVALID_EXPECTED" && "记录校验值无效", r.preview === "FAIL" && "预览失败"].filter(Boolean);
  const passed = r.boundary === "SAFE" && r.existence === "PRESENT" && r.readability === "READABLE" && r.checksum === "MATCH" && ["PASS", "NOT_APPLICABLE"].includes(r.preview);
  return { issue: issues.length > 0, incomplete: !passed && !issues.length, label: issues.length ? issues.join(" · ") : passed ? "检查通过" : r.preview === "CHECK_UNAVAILABLE" ? "部分检查未完成 · 预览检查不可用" : "部分检查未完成" };
}
const initial = (projectId: string): ScanState => ({ projectId, checked: 0, issues: 0, incomplete: 0, results: [], phase: "IDLE" });

// One transient coordinator for this mounted Library. Explicit actions only;
// no polling, persistence, new store or repair/relink/import mutation.
export function useLibraryMediaInspection(projectId: string, resource?: ResourceRef) {
  const [scan, setScan] = useState<ScanState>(() => initial(projectId));
  const [single, setSingle] = useState<{ owner: string; result: MediaCheckResult }>();
  const [busy, setBusy] = useState(false);
  const active = useRef(false), stopped = useRef(false), epoch = useRef(0), mounted = useRef(true);
  const liveProject = useRef(projectId); liveProject.current = projectId;
  const assetId = resource?.kind === "asset" ? resource.id : undefined;
  const owner = `${projectId}:${assetId ?? ""}`;
  const liveOwner = useRef(owner), singleEpoch = useRef(0);
  if (liveOwner.current !== owner) { ++singleEpoch.current; liveOwner.current = owner; }
  useEffect(() => { setSingle(undefined); }, [owner]);
  useEffect(() => { stopped.current = true; ++epoch.current; setScan(initial(projectId)); setSingle(undefined); }, [projectId]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; stopped.current = true; ++epoch.current; }; }, []);
  const finish = () => { active.current = false; if (mounted.current) setBusy(false); };
  async function verifySingle(title: string) {
    if (active.current || !assetId) return;
    active.current = true; setBusy(true); setSingle(undefined);
    const requestOwner = owner, requestEpoch = singleEpoch.current;
    try {
      const report = await productClient.library.mediaVerify(projectId, { kind: "asset", id: assetId });
      if (mounted.current && liveOwner.current === requestOwner && singleEpoch.current === requestEpoch) setSingle({ owner: requestOwner, result: { assetId, title, report } });
    } catch (error) {
      if (mounted.current && liveOwner.current === requestOwner && singleEpoch.current === requestEpoch) setSingle({ owner: requestOwner, result: { assetId, title, error: normalizeProductError(error).message } });
    } finally { finish(); }
  }
  async function startScan() {
    if (active.current) return;
    active.current = true; stopped.current = false; setBusy(true); setSingle(undefined);
    const token = ++epoch.current;
    const owns = () => mounted.current && liveProject.current === projectId && epoch.current === token;
    setScan({ ...initial(projectId), phase: "RUNNING" });
    let cursor: LibraryCursor | null = null;
    try {
      do {
        if (!owns() || stopped.current) break;
        const page = await productClient.library.list(projectId, { category: "media", keyword: null, favoriteOnly: false, tagId: null, cursor, limit: SCAN_PAGE_SIZE });
        if (!owns() || stopped.current) break;
        for (const item of page.items) {
          if (!owns() || stopped.current) break;
          if (item.resourceRef.kind !== "asset") continue;
          setScan(s => ({ ...s, current: item.title }));
          const result: MediaCheckResult = { assetId: item.resourceRef.id, title: item.title };
          try { result.report = await productClient.library.mediaVerify(projectId, item.resourceRef); }
          catch (error) { result.error = normalizeProductError(error).message; }
          if (!owns()) break;
          const summary = result.report ? mediaCheckSummary(result.report) : { issue: false, incomplete: true };
          setScan(s => ({ ...s, checked: s.checked + 1, issues: s.issues + Number(summary.issue), incomplete: s.incomplete + Number(summary.incomplete), results: [...s.results, result].slice(-SCAN_RESULT_LIMIT) }));
        }
        if (page.nextCursor && JSON.stringify(page.nextCursor) === JSON.stringify(cursor)) throw new Error("扫描分页未推进，请重新检查。");
        cursor = page.nextCursor;
      } while (cursor && owns() && !stopped.current);
      if (owns()) setScan(s => ({ ...s, current: undefined, phase: stopped.current ? "STOPPED" : "COMPLETED" }));
    } catch (error) { if (owns()) setScan(s => ({ ...s, current: undefined, phase: "STOPPED", error: normalizeProductError(error).message })); }
    finally { finish(); }
  }
  function stopScan() { stopped.current = true; setScan(s => s.phase === "RUNNING" ? { ...s, phase: "STOPPING" } : s); }
  return { busy, single: single?.owner === owner ? single.result : undefined, scan: scan.projectId === projectId ? scan : initial(projectId), verifySingle, startScan, stopScan };
}
export type LibraryMediaInspection = ReturnType<typeof useLibraryMediaInspection>;
