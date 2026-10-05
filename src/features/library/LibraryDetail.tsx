import { SingleMediaInspection } from "./MediaIntegrityPanel";
import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { LibraryDetail as Detail, LibraryEditRequest } from "../../product/libraryTypes";
import type { LibraryController, LibraryProps } from "./LibraryController";
import { detailTitle, profileValue, relationRoute, subtypeLabel, resourceKey, deletionMessage } from "./libraryModel";
function MediaPreview({projectId,detail}:{projectId:string;detail:Extract<Detail,{kind:"asset"}>}) {
  const [image,setImage]=useState<string>();const [failed,setFailed]=useState(false);const a=detail.asset;
  useEffect(()=>{let alive=true;let objectUrl:string|undefined;setImage(undefined);setFailed(false);
    if(a.assetType==="image")void productClient.library.imageGet(projectId,{kind:"asset",id:a.id}).then(bytes=>{if(!alive)return;objectUrl=URL.createObjectURL(new Blob([Uint8Array.from(bytes)],{type:a.mimeType}));setImage(objectUrl);}).catch(()=>{if(alive)setFailed(true);});
    return()=>{alive=false;if(objectUrl)URL.revokeObjectURL(objectUrl);};
  },[projectId,a.id,a.assetType,a.mimeType]);
  if(failed)return <p role="status">媒体无法预览；使用关系和历史仍可查看。</p>;
  if(a.assetType==="image")return image?<img className="library-preview" src={image} alt={a.name} onError={()=>setFailed(true)}/>:<p role="status">正在读取图片…</p>;
  return a.assetType==="audio"?<audio controls preload="metadata" src={productClient.library.mediaUrl(projectId,a.id,"audio")} onError={()=>setFailed(true)}/>:<video className="library-preview" controls preload="metadata" src={productClient.library.mediaUrl(projectId,a.id,"video")} onError={()=>setFailed(true)}/>;
}
function EditForm({detail,c,close}:{detail:Detail;c:LibraryController;close:()=>void}) {
  const [name,setName]=useState(detailTitle(detail));
  const latest=detail.kind==="prompt"?[...detail.prompt.versions].sort((a,b)=>b.version-a.version)[0]:undefined;
  const [text,setText]=useState(latest?.text??"");const [description,setDescription]=useState(detail.kind==="reference-set"?detail.referenceSet.description:"");
  const [members,setMembers]=useState(detail.kind==="reference-set"?detail.referenceSet.items:[]);
  async function save(){let request:LibraryEditRequest;
    if(detail.kind==="prompt")request={kind:"prompt",id:detail.prompt.id,text,modelVersionId:latest?.modelVersionId??null};
    else if(detail.kind==="profile")request={kind:"profile",id:profileValue(detail).id,name};
    else if(detail.kind==="reference-set")request={kind:"reference-set",id:detail.referenceSet.id,name,description,items:members.map((v,ordinal)=>({assetId:v.assetId,ordinal,role:v.role,isPrimary:v.isPrimary}))};else return;
    if(await c.edit(request))close();
    // Failed writes and background refreshes preserve the user form.
  }
  return <form className="library-editor" onSubmit={e=>{e.preventDefault();void save();}}><h3>编辑资源</h3>{detail.kind==="prompt"?<><label>新版本正文<textarea aria-label="新版本正文" value={text} onChange={e=>setText(e.target.value)} rows={8}/></label><small>保存将创建新版本，旧正文和版本不会被覆盖。</small></>:<label>名称<input aria-label="资源名称" value={name} onChange={e=>setName(e.target.value)}/></label>}
    {detail.kind==="reference-set"&&<><label>说明<textarea value={description} onChange={e=>setDescription(e.target.value)}/></label><ol>{members.map((m,index)=><li key={m.assetId}><span>{m.assetName}</span><label>成员角色<input value={m.role??""} onChange={e=>setMembers(members.map((v,i)=>i===index?{...v,role:e.target.value||null}:v))}/></label>{index>0&&<button type="button" onClick={()=>{const next=[...members];[next[index-1],next[index]]=[next[index],next[index-1]];setMembers(next);}}>上移</button>}<button type="button" onClick={()=>setMembers(members.filter((_,i)=>i!==index))}>移除成员</button></li>)}</ol><small>用途与所属设定保持不变；添加成员及复杂配置请用高级编辑。</small></>}
    <button type="submit" disabled={c.busy}>保存{detail.kind==="prompt"?"新版本":"修改"}</button><button type="button" disabled={c.busy} onClick={close}>取消编辑</button></form>;
}
export function LibraryDetailPanel({controller:c,route,navigate}:LibraryProps&{controller:LibraryController}) {
  const [editing,setEditing]=useState(false);const d=c.detail;
  useEffect(()=>{setEditing(false);},[route.projectId,resourceKey(route.resource)]);
  if(!d)return <section className="library-detail"><p>选择资源查看预览与使用位置。</p></section>;
  const source=c.relations.find(r=>r.kind==="GENERATION_OUTPUT"&&r.location?.kind==="run");
  return <section className="library-detail" aria-label="资源详情"><h2>{detailTitle(d)}</h2>
    {d.kind==="asset"&&<><MediaPreview projectId={route.projectId} detail={d}/><SingleMediaInspection inspection={c.mediaInspection} title={d.asset.name}/><p>{subtypeLabel(d.asset.assetType??"")} · {d.asset.width??0} × {d.asset.height??0}</p><p>{d.asset.sourceTaskId?"生成结果":"导入素材"}</p>{source?.location&&<button type="button" onClick={()=>navigate(relationRoute(route.projectId,source.location!))}>查看来源运行</button>}</>}
    {d.kind==="prompt"&&<><h3>当前正文</h3><p className="library-prompt-text">{[...d.prompt.versions].sort((a,b)=>b.version-a.version)[0]?.text??"暂无正文"}</p><h3>版本历史</h3>{c.versions?.kind==="prompt"&&c.versions.versions.map((v,index)=><details key={v.id} open={index===0}><summary>版本 {v.version}{index===0?"（当前）":""}</summary><p className="library-prompt-text">{v.text}</p></details>)}</>}
    {d.kind==="profile"&&<><h3>{subtypeLabel("Character"in d.profile?"CHARACTER":"Scene"in d.profile?"SCENE":"Prop"in d.profile?"PROP":"STYLE")}</h3><dl>{Object.entries(profileValue(d)).filter(([key])=>["description","canonical_prompt","negative_prompt","environment_prompt","lighting_prompt","material_prompt","scale_prompt","style_prompt","color_prompt","line_prompt","output_notes"].includes(key)).map(([key,value])=><div key={key}><dt>{({description:"说明",canonical_prompt:"主体描述",negative_prompt:"排除内容",environment_prompt:"环境",lighting_prompt:"光照",material_prompt:"材质",scale_prompt:"尺度",style_prompt:"风格",color_prompt:"色彩",line_prompt:"线条",output_notes:"输出说明"} as Record<string,string>)[key]}</dt><dd>{value||"未设置"}</dd></div>)}</dl></>}
    {d.kind==="reference-set"&&<><p>{d.referenceSet.description}</p><h3>有序成员</h3><ol>{d.referenceSet.items.map(m=><li key={m.assetId}><button type="button" onClick={()=>navigate({kind:"library",projectId:route.projectId,resource:{kind:"asset",id:m.assetId}})}>{m.assetName}</button><span>{m.role??"未指定角色"}{m.isPrimary?" · 主参考":""}</span></li>)}</ol></>}
    {d.kind==="prompt"&&<p>用于创作时使用当前最新版本。</p>}
    <div className="library-actions">{d.kind==="asset"&&d.asset.assetType==="audio"?<p>当前普通图片/H3创作未启用音频输入；资源仍保留在资源库。</p>:c.returnRoute?<button type="button" disabled={c.busy} onClick={()=>void c.useInCreation()}>用于创作{d.kind==="prompt"?"（使用当前最新版本）":""}</button>:<>{(d.kind!=="asset"||d.asset.assetType==="image")&&<button type="button" disabled={c.busy} onClick={()=>void c.useInCreation("image")}>用于图片创作</button>}<button type="button" disabled={c.busy} onClick={()=>void c.useInCreation("video")}>用于视频创作</button></>}{d.kind!=="asset"&&<button type="button" disabled={c.busy} onClick={()=>setEditing(!editing)}>编辑</button>}<button type="button" disabled={c.busy} onClick={()=>void c.inspectDelete()}>删除</button><button type="button" onClick={()=>navigate({...route,filter:d.kind==="prompt"?"advanced-prompts":"advanced-assets"})}>高级编辑</button></div>
    {editing&&<EditForm detail={d} c={c} close={()=>setEditing(false)}/>}
    <h3>使用位置</h3>{source?.location&&c.relations.some(r=>r.kind==="ARTIFACT_REVIEW_MEANINGFUL")&&<button type="button" onClick={()=>navigate(relationRoute(route.projectId,source.location!))}>查看审核信息</button>}{c.relations.length?<ul className="library-relations">{c.relations.map((r,index)=><li key={index}><strong>{r.title}</strong><span>{r.blocking?"受保护的使用关系":"历史 / 关联信息"}</span>{r.kind==="GENERATION_SNAPSHOT_INPUT"&&<p>历史生成任务固定输入，必须保留以支持输入复现与精确重试。</p>}{r.location&&<button type="button" onClick={()=>navigate(relationRoute(route.projectId,r.location!))}>{r.location.kind==="shot"?"查看镜头":r.location.kind==="run"?"查看运行":"查看关联资源"}</button>}<details><summary>关系详情</summary><p>{r.description}</p></details></li>)}</ul>:<p>未发现正式使用关系。</p>}
    {c.inspection&&<section className="library-delete-inspection" aria-label="删除影响"><h3>{c.inspection.allowed?"确认删除资源":"无法删除"}</h3>{[...c.inspection.blockers,...c.inspection.warnings,...c.inspection.consequences].map((s,i)=><p key={i}>{deletionMessage(s)}</p>)}{c.inspection.allowed&&<button type="button" disabled={c.busy} onClick={()=>void c.confirmDelete()}>确认删除资源</button>}<button type="button" disabled={c.busy} onClick={c.closeInspection}>返回</button></section>}
  </section>;
}
