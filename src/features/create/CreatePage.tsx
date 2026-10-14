import { useCreateController, type CreateProps } from "./CreateController";
import { GeneratorPanel, MediaInputPanel, ParameterPanel, PromptPanel, ReferencePanel } from "./CreateInputs";
import { CandidatePanel, GenerateBar } from "./CreateResults";
import { CreateComposer } from "./CreateComposer";
import { fieldLabel } from "./createModel";
import "./CreatePage.css";
import { CreateVideoWorkspace } from "./CreateVideoWorkspace";
export function CreatePage(props: CreateProps) {
  const c = useCreateController(props);
  if (c.loading) return <div className="create-page" role="status">正在载入创作上下文…</div>;
  if (!c.context) return <div className="create-page"><p role="alert">{c.error ?? "无法读取项目"}</p></div>;
  if (props.route.stage === "image") return <section className="create-page create-history" aria-label="历史图片查看">
    <header className="create-header"><h1>历史图片</h1><label>镜头<select aria-label="镜头" value={props.route.shotId ?? ""} disabled={c.busy} onChange={e => c.selectShot(e.target.value)}><option value="">请选择镜头</option>{c.context.shots.map(shot => <option key={shot.id} value={shot.id}>{shot.name}</option>)}</select></label></header>
    <p role="note">此入口仅保留历史图片查看与既有结果选用，不再提供新图片生成。外部图片可用于视频创作。</p>
    <div><button type="button" onClick={() => props.navigate({ ...props.route, stage: "video", surface: undefined })}>视频创作</button><button type="button" onClick={() => props.navigate({ kind: "runs", projectId: props.route.projectId, filter: "tasks", context: { shotId: props.route.shotId, stage: "IMAGE" } })}>历史运行与来源</button></div>
    {c.error && <p role="alert">{c.error}</p>}
    {c.context.selectedShot ? <CandidatePanel controller={c} route={props.route} /> : <p>选择镜头查看历史结果。</p>}
  </section>;
  if (!c.context.shots.length) return <div className="create-page create-empty"><h1>开始创作</h1><p>在当前项目中创建第一个镜头。</p>{c.error && <p role="alert">{c.error}</p>}<button className="primary" disabled={c.busy} onClick={() => void c.createShot()}>创建第一个镜头</button></div>;
  return <div className="create-page create-video-workbench"><header className="create-header"><button type="button" className="create-back" onClick={() => void c.openStoryboard()}>← 分镜工作台</button><div><h1>视频创作</h1><small>{c.context.projectName} / {c.context.selectedShot?.summary.name ?? "单镜头"}</small></div><label>镜头<select aria-label="镜头" value={props.route.shotId ?? ""} disabled={c.busy || c.videoInputs.busy} onChange={e => c.selectShot(e.target.value)}><option value="">请选择镜头</option>{c.context.shots.map(shot => <option key={shot.id} value={shot.id}>{shot.name}</option>)}</select></label>
    <button type="button" disabled={c.busy} onClick={() => void c.createShot()}>新建镜头</button><span>MiniMax 视频</span>
    <details className="create-advanced"><summary>高级</summary><button type="button" onClick={() => c.openAdvanced("batch")}>批量镜头 / 外部素材绑定</button><button type="button" onClick={() => c.openAdvanced("production")}>生产包 / 批量导入</button><button type="button" onClick={() => c.openAdvanced("workflows")}>工作流 / Benchmark</button></details></header>
    {c.libraryIntent && <section className="create-library-intent" aria-label="资源库复用意图"><h2>来自资源库 · 绑定到正式输入</h2>{c.libraryIntent.kind === "asset" ? <><p>选择目标镜头的明确输入槽；写入持久视频输入，不创建任务、不启动生成，也不改长期参考或已冻结批次。</p>{c.libraryNeedsGeneratedVideoConfirm&&<label><input type="checkbox" checked={c.generatedVideoConfirm} onChange={e=>c.setGeneratedVideoConfirm(e.target.checked)}/>确认将生成视频仅作为视频参考绑定（不抽帧为首帧）</label>}{c.librarySlots.map(field=><button type="button" key={field.key} disabled={c.busy || !c.context?.selectedShot || !c.libraryApplyReady || (c.libraryNeedsGeneratedVideoConfirm && !c.generatedVideoConfirm)} onClick={()=>void c.applyLibraryAsset(field.key)}>应用到{fieldLabel(field)}</button>)}{c.libraryRejectReason&&<p role="status">{c.libraryRejectReason}</p>}{!c.librarySlots.length&&!c.libraryRejectReason&&<p>当前生成器没有兼容输入，请明确选择其他生成器。</p>}</> : c.libraryIntent.kind === "context" ? <p>{c.libraryIntent.message}，尚未自动应用。</p> : <p>当前生成器没有可应用提示词的输入，请明确选择其他生成器；选择镜头后才能应用。</p>}</section>}
    {!c.context.selectedShot ? <p>选择一个镜头开始创作。</p> : <><div className="create-content"><main id="create-inputs" tabIndex={-1} aria-label="创作输入" className="create-inputs"><fieldset disabled={c.busy}><GeneratorPanel controller={c} /><PromptPanel controller={c} /><MediaInputPanel controller={c} />{!c.videoInputs.enabled && <ReferencePanel context={c.context} save={c.setReferences} busy={c.busy} />}<ParameterPanel controller={c} /></fieldset></main><CreateVideoWorkspace key={JSON.stringify([props.route.projectId,props.route.shotId,c.selection])} controller={c} route={props.route} /></div><CreateComposer controller={c} /><GenerateBar controller={c} /></>}
  </div>;
}
