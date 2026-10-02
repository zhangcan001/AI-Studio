import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { RunResult } from "../../product/types";
import type { RunsProps, RunsController } from "./RunsController";
function Thumbnail({ bytes }: { bytes: number[] }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => { const url = URL.createObjectURL(new Blob([Uint8Array.from(bytes)], { type: "image/png" })); setUrl(url); return () => URL.revokeObjectURL(url); }, [bytes]);
  return <img src={url} alt="生成结果" />;
}
const reviewLabels: Record<string, string> = { PENDING: "待审核", APPROVED: "已通过", REJECTED: "已拒绝" };
function ReviewActions({ result, controller: c }: { result: RunResult; controller: RunsController }) {
  const [comment, setComment] = useState("");
  return <div className="run-review-actions"><label>审核说明<input value={comment} onChange={event => setComment(event.target.value)} disabled={c.busy} /></label><button type="button" disabled={c.busy} onClick={() => void c.review(result, "APPROVED", comment)}>审核通过</button><button type="button" disabled={c.busy || !comment.trim()} onClick={() => void c.review(result, "REJECTED", comment)}>审核拒绝</button></div>;
}
export function RunResults({ controller: c, route, navigate }: RunsProps & { controller: RunsController }) {
  return <section><h3>结果</h3><p>执行成功、素材存在、审核通过与镜头选用是四个独立事实。</p><div className="run-results">{c.results.map(result => <article key={result.assetId}>
    {result.thumbnailBytes && <Thumbnail bytes={result.thumbnailBytes} />}
    {result.mediaKind === "video" && result.assetExists && result.availability === "available" && <video controls preload="metadata" src={productClient.creation.mediaUrl(route.projectId, result.assetId)} />}
    <strong>{result.name}</strong><p>{result.assetExists ? result.availability === "missing" ? "素材文件缺失" : "素材存在" : "素材不存在"} · {result.reviewState ? reviewLabels[result.reviewState] ?? "审核状态未知" : "无审核记录"} · {result.selectedShotIds.length ? "镜头已选用" : "镜头未选用"}</p>
    <button type="button" disabled={!result.assetExists} onClick={() => navigate({ kind: "library", projectId: route.projectId, resource: { kind: "asset", id: result.assetId } })}>在素材库查看</button>
    {result.reviewRevision !== null && result.assetExists && result.reviewState === "PENDING" && <ReviewActions result={result} controller={c} />}
  </article>)}</div>{!c.results.length && <p>暂无结果。不会把任务成功当作素材已存在。</p>}</section>;
}
