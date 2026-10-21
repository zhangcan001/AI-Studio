import { useEffect, useRef, useState } from "react";
import { useFolderBatchImportController } from "./useFolderBatchImportController";


export interface FolderBatchImportProps {
  projectId: string;
  onImported: () => unknown;
  onCancel: () => void;
  onOpenQueue?: (batchId: string) => void;
  onOpenLegacy?: () => void;
}
const modes = { image: "单图视频", firstLast: "首尾帧", reference: "混合参考" };
export function FolderBatchImportPanel(props: FolderBatchImportProps) {
  const c = useFolderBatchImportController(props.projectId, props.onImported);
  const { preview: p, result: r } = c;
  const survivingBatch=r?.outcome==="completed"?r.batchId:r?.residual.batchId;
  const [page,setPage]=useState(0);useEffect(()=>setPage(0),[p?.previewVersion,p?.sessionId]);
  const pageCount=Math.ceil((p?.rows.length??0)/50);
  const generator=c.generators.find(g=>g.selectionRef===c.legacy.selectionRef);
  return <section className="folder-import" aria-label="文件夹批量导入" aria-busy={Boolean(c.busy)}>
    <header><div><h2>从文件夹导入分镜</h2><p>先校验，再导入素材与镜头。创建批次不等于开始生成。</p></div><button type="button" onClick={props.onCancel} disabled={c.busy === "commit"}>关闭</button></header>
    <div className="folder-import-actions"><button type="button" disabled={Boolean(c.busy)} onClick={() => void c.pick()}>{c.busy === "pick" ? "正在选择…" : p ? "重新选择文件夹" : "选择文件夹"}</button>{props.onOpenLegacy && <button type="button" disabled={Boolean(c.busy)} onClick={props.onOpenLegacy}>旧版脚本与高级生产</button>}</div>
    {!p && <p>文件夹需包含 manifest.json 和引用图片；不支持 ZIP，不会改动源文件。</p>}
    {c.error && <p className="folder-import-error" role="alert">{c.error}</p>}
    {c.busy && <p role="status">{c.busy === "commit" ? "正在分步导入，请勿关闭窗口；失败时将报告补偿结果。" : "正在安全扫描并校验正式生成器…"}</p>}
    {p && <>
      <dl className="folder-import-summary"><div><dt>文件夹</dt><dd>{p.displayRootName}</dd></div><div><dt>清单镜头</dt><dd>{p.shotCount}</dd></div><div><dt>可导入</dt><dd>{p.validCount}</dd></div><div><dt>存在错误</dt><dd>{p.errorCount}</dd></div></dl>
      {p.errors.map((e, i) => <p key={i} className="folder-import-error" role="alert">{e.code}：{e.message}</p>)}
      {p.warnings.map((w, i) => <p key={i} role="note">{w}</p>)}
      {c.legacyMode && <fieldset disabled={Boolean(c.busy)||Boolean(r)}><legend>旧版清单：明确选择正式生成器及默认参数</legend><label>生成器<select value={c.legacy.selectionRef} onChange={e=>{const option=c.generators.find(g=>g.selectionRef===e.target.value);const preset=option?.resolutionPresets?.[0];c.setLegacy({...c.legacy,selectionRef:e.target.value,width:preset?.width??0,height:preset?.height??0});}}><option value="">请选择，不按名称推断身份</option>{c.generators.map(g=><option key={g.selectionRef} value={g.selectionRef} disabled={!g.availability}>{g.name} · {g.version} · {g.mode}{g.availability?"":"（不可用）"}</option>)}</select></label><label>默认时长（秒）<input type="number" min={1} max={15} value={c.legacy.durationSeconds} onChange={e=>c.setLegacy({...c.legacy,durationSeconds:Number(e.target.value)})}/></label><label>正式分辨率<select value={generator?.resolutionPresets?.find(v=>v.width===c.legacy.width&&v.height===c.legacy.height)?.id??""} onChange={e=>{const preset=generator?.resolutionPresets?.find(v=>v.id===e.target.value);if(preset)c.setLegacy({...c.legacy,width:preset.width,height:preset.height});}}><option value="">请选择正式档位</option>{generator?.resolutionPresets?.map(v=><option key={v.id} value={v.id}>{v.label} · {v.width} × {v.height}</option>)}</select></label><p>选择后点“重新校验”；模式和输入限制由正式后端判断，不会只凭生成器名称接受。</p></fieldset>}
      <div className="folder-import-actions"><label>镜头名称后缀<input value={c.suffix} maxLength={80} disabled={Boolean(c.busy) || Boolean(r)} onChange={e => c.setSuffix(e.target.value)} placeholder="再次导入需填写后缀" /></label><button type="button" disabled={Boolean(c.busy) || Boolean(r)} onClick={() => void c.rescan()}>重新校验</button>{c.dirty && <span role="status">预览参数已修改，需重新校验才能导入。</span>}</div>
      {p.characters.length > 0 && <details><summary>角色与场景计划（{p.characters.length}）</summary><ul>{p.characters.map(e => <li key={`${e.kind}:${e.name}`}>{e.name} · {e.kind === "scene" ? "场景" : "角色"} · {e.action === "reuse" ? "只读复用" : `新建，${e.imageCount} 张图`}</li>)}</ul></details>}
      <div className="folder-import-table" tabIndex={0} aria-label="导入镜头预览"><table><thead><tr><th>选择</th><th>镜头与提示词</th><th>模式 / 生成器</th><th>参数</th><th>输入</th><th>校验结果</th></tr></thead><tbody>{p.rows.slice(page*50,(page+1)*50).map(row => <tr key={row.rowKey}><td><input type="checkbox" aria-label={`导入 ${row.name}`} checked={c.selected.has(row.rowKey)} disabled={Boolean(c.busy) || Boolean(r) || row.errors.length > 0} onChange={() => c.toggle(row.rowKey)} /></td><td><strong>{row.rowNumber}. {row.name}</strong><p>{row.promptPreview}</p></td><td>{modes[row.mode]}<small>{row.generatorLabel}</small></td><td>{row.durationSeconds}s<small>{row.width} × {row.height}</small></td><td>{row.firstFrame && <small>首帧：{row.firstFrame}</small>}{row.lastFrame && <small>尾帧：{row.lastFrame}</small>}{row.mode === "reference" && <small>参考图：{row.referenceCount} 张</small>}{row.entityNames.join("、")}</td><td>{row.errors.length === 0 ? <span>可导入</span> : row.errors.map((e, i) => <p className="folder-import-error" key={i}>{e.code}：{e.message}</p>)}{row.warnings.map((w, i) => <p key={i}>{w.code}：{w.message}</p>)}</td></tr>)}</tbody></table></div>{pageCount>1 && <nav aria-label="导入预览分页"><button type="button" disabled={page===0} onClick={()=>setPage(n=>n-1)}>上一页</button><span>{page+1} / {pageCount}</span><button type="button" disabled={page+1>=pageCount} onClick={()=>setPage(n=>n+1)}>下一页</button></nav>}
      {!r && <footer><p>已选 {c.selected.size} / {p.shotCount}；不导入其余 {p.shotCount - c.selected.size} 个镜头及其独占素材。</p><label><input type="checkbox" checked={c.createQueue} disabled={Boolean(c.busy)} onChange={e => c.setCreateQueue(e.target.checked)} />创建待启动批次</label><label><input type="checkbox" checked={c.autoStart} disabled={Boolean(c.busy) || !c.createQueue} onChange={e => c.setAutoStart(e.target.checked)} />导入后立即开始生产（占用 GPU，需明确勾选）</label>{c.overQueueLimit && <p className="folder-import-error">正式单批最多 {p.maxQueueItems} 项；请减少选择，或取消创建批次。</p>}<button type="button" className="primary" disabled={!c.canCommit} onClick={() => void c.commit()}>{c.busy === "commit" ? "正在导入…" : c.autoStart ? "导入并开始生产" : "确认导入"}</button></footer>}
      {r && <section aria-label="导入最终回执" role="status"><h3>{r.outcome === "completed" ? "导入完成" : r.outcome === "compensated" ? "导入失败，已补偿本次新增数据" : "导入未完成，有数据残留"}</h3><p>本轮创建镜头 {r.createdShotIds.length}、实体 {r.createdEntityIds.length}、素材 {r.importedAssetIds.length}；批次条目 {r.itemCount}。{r.autoStarted ? "已请求正式队列启动。" : "没有自动启动生产。"}</p>{r.failure && <p className="folder-import-error">{r.failure.stage} / {r.failure.code}：{r.failure.message}</p>}{r.compensationErrors.map((e, i) => <p className="folder-import-error" key={i}>{e.target} {e.id}：{e.message}</p>)}{r.outcome === "partial" && <pre>{JSON.stringify(r.residual, null, 2)}</pre>}{r.warnings.map((w, i) => <p key={i}>{w}</p>)}{survivingBatch && props.onOpenQueue && <button type="button" onClick={() => props.onOpenQueue?.(survivingBatch!)}>查看生产队列</button>}<p>此会话已结束，不会重复写入。再次导入请选择文件夹并重新校验。</p></section>}
    </>}
  </section>;
}
/** Native dialog supplies focus trapping and Escape, retaining the workbench draft. */
export function FolderBatchImportDialog(props: FolderBatchImportProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); }, []);
  return <dialog ref={ref} className="folder-import-dialog" aria-label="文件夹批量导入" onCancel={e => { e.preventDefault(); /* Close is explicitly disabled while committing. */ ref.current?.querySelector<HTMLButtonElement>(".folder-import header button:not(:disabled)")?.click(); }}><FolderBatchImportPanel {...props} /></dialog>;
}
