import type { MediaIntegrityReport } from "../../product/libraryTypes";
import { mediaCheckSummary, type LibraryMediaInspection } from "./useLibraryMediaInspection";
const labels: Record<string, string> = { SAFE: "安全", REJECTED: "安全边界拒绝", NOT_CHECKED: "未检查", PRESENT: "存在", MISSING: "文件缺失", READABLE: "可读取", UNREADABLE: "不可读取", MATCH: "一致", MISMATCH: "校验不一致", INVALID_EXPECTED: "记录校验值无效", PASS: "通过", FAIL: "预览失败", CHECK_UNAVAILABLE: "预览检查不可用", NOT_APPLICABLE: "不适用" };
export function MediaIntegrityFacts({ report: r }: { report: MediaIntegrityReport }) {
  return <section aria-label="媒体检查结果"><strong>{mediaCheckSummary(r).label}</strong><dl>{[["安全边界",r.boundary],["文件存在",r.existence],["文件读取",r.readability],["SHA-256校验",r.checksum],["预览探测",r.preview]].map(([label,value])=><div key={label}><dt>{label}</dt><dd>{labels[value]}</dd></div>)}</dl><time dateTime={r.checkedAt}>{new Date(r.checkedAt).toLocaleString("zh-CN")}</time>
    {r.existence === "MISSING" && <p>受管理媒体文件未找到。当前版本不会自动删除资源记录或关系。可查看使用位置，并从原始来源重新导入为新资源。</p>}
    {r.readability === "UNREADABLE" && <p>文件存在，但当前进程无法读取。请检查文件访问权限或项目存储状态。</p>}
    {r.checksum === "MISMATCH" && <p>文件内容与记录的 SHA-256 不一致。当前版本不会修改校验值，也不会把该文件自动认定为原资源的新版本。</p>}
    {r.checksum === "INVALID_EXPECTED" && <p>记录中的校验值格式无效，尚不能判定内容是否一致；当前检查不会修改记录。</p>}
    {r.preview === "FAIL" && <p>文件存在且完整性检查结果独立于预览能力。预览探测失败可能与媒体格式或 codec 支持有关；资源记录和历史关系仍保留。</p>}
    {r.preview === "CHECK_UNAVAILABLE" && <p>当前环境或检查上限无法执行媒体预览探测。这不代表媒体文件损坏。</p>}
    {r.boundary === "REJECTED" && <p>受管理文件安全边界未通过；未继续读取或校验文件。</p>}
  </section>;
}
export function SingleMediaInspection({ inspection, title }: { inspection: LibraryMediaInspection; title: string }) {
  return <section aria-label="媒体完整性检查"><button type="button" disabled={inspection.busy} onClick={()=>void inspection.verifySingle(title)}>检查此媒体</button><p>仅检查 AI Studio 受管理副本，不检查原始导入位置；结果只保留在本次界面中。</p>{inspection.single?.report && <MediaIntegrityFacts report={inspection.single.report}/>} {inspection.single?.error && <p role="alert">{inspection.single.error}</p>}</section>;
}
export function ProjectMediaInspection({ inspection, open }: { inspection: LibraryMediaInspection; open: (id: string) => void }) {
  const s = inspection.scan;
  return <section aria-label="项目媒体检查"><button type="button" disabled={inspection.busy} onClick={()=>void inspection.startScan()}>检查当前项目媒体</button><p>检查当前项目全部受管理媒体，不受搜索、收藏或标签筛选限制。只读检查，不会修复或删除文件。</p>{["RUNNING","STOPPING"].includes(s.phase) && <button type="button" disabled={s.phase === "STOPPING"} onClick={inspection.stopScan}>停止检查</button>}
    {s.phase !== "IDLE" && <><p role="status">已检查 {s.checked} 项 · 已发现问题 {s.issues} 项 · 部分检查未完成 {s.incomplete} 项{s.current ? ` · 正在检查：${s.current}` : ""} · {({RUNNING:"检查中",STOPPING:"正在停止，当前媒体允许完成",STOPPED:"已停止",COMPLETED:"检查完成",IDLE:"未开始"})[s.phase]}</p><p>停止在媒体之间生效；下方仅显示最近50项检查结果，结果未保存。</p>{s.error&&<p role="alert">{s.error}</p>}<ul>{s.results.map((r,index)=><li key={`${r.assetId}:${index}`}><button type="button" onClick={()=>open(r.assetId)}>{r.title} · 查看使用位置</button><span>{r.report ? mediaCheckSummary(r.report).label : "检查未完成"}</span>{r.report&&<details><summary>检查事实</summary><MediaIntegrityFacts report={r.report}/></details>}{r.error&&<p>{r.error}</p>}</li>)}</ul></>}
  </section>;
}
