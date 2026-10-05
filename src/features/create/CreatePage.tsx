import { useCreateController, type CreateProps } from "./CreateController";
import { GeneratorPanel, MediaInputPanel, ParameterPanel, PromptPanel, ReferencePanel } from "./CreateInputs";
import { CandidatePanel, GenerateBar, RunStatusCard } from "./CreateResults";
import { fieldLabel } from "./createModel";
import "./CreatePage.css";
export function CreatePage(props: CreateProps) {
  const c = useCreateController(props);
  if (c.loading) return <div className="create-page" role="status">正在载入创作上下文…</div>;
  if (!c.context) return <div className="create-page"><p role="alert">{c.error ?? "无法读取项目"}</p></div>;
  if (!c.context.shots.length) return <div className="create-page create-empty"><h1>开始创作</h1><p>在当前项目中创建第一个镜头。</p>{c.error && <p role="alert">{c.error}</p>}<button className="primary" disabled={c.busy} onClick={() => void c.createShot()}>创建第一个镜头</button></div>;
  return <div className="create-page"><header className="create-header"><h1>创作</h1><label>镜头<select aria-label="镜头" value={props.route.shotId ?? ""} disabled={c.busy} onChange={e => c.selectShot(e.target.value)}><option value="">请选择镜头</option>{c.context.shots.map(shot => <option key={shot.id} value={shot.id}>{shot.name}</option>)}</select></label>
    <button type="button" disabled={c.busy} onClick={() => void c.createShot()}>新建镜头</button><nav aria-label="创作阶段"><button type="button" aria-pressed={props.route.stage === "image"} disabled={c.busy} onClick={() => c.selectStage("image")}>图片</button><button type="button" aria-pressed={props.route.stage === "video"} disabled={c.busy} onClick={() => c.selectStage("video")}>视频</button></nav>
    <details className="create-advanced"><summary>高级</summary><button type="button" onClick={() => c.openAdvanced("batch")}>批量生成 / Experiment</button><button type="button" onClick={() => c.openAdvanced("production")}>生产包 / 批量导入</button><button type="button" onClick={() => c.openAdvanced("workflows")}>工作流 / Benchmark</button></details></header>
    {c.libraryIntent && <section aria-label="资源库复用意图"><h2>来自资源库</h2>{c.libraryIntent.kind === "asset" ? <><p>请选择本次草稿的明确输入位置；不会修改镜头长期参考。</p>{c.librarySlots.map(field=><button type="button" key={field.key} disabled={c.busy || !c.context?.selectedShot} onClick={()=>c.applyLibraryAsset(field.key)}>应用到{fieldLabel(field)}</button>)}{!c.librarySlots.length&&<p>当前生成器没有兼容输入，请明确选择其他生成器。</p>}</> : c.libraryIntent.kind === "context" ? <p>{c.libraryIntent.message}，尚未自动应用。</p> : <p>当前生成器没有可应用提示词的输入，请明确选择其他生成器；选择镜头后才能应用。</p>}</section>}
    {!c.context.selectedShot ? <p>选择一个镜头开始创作。</p> : <><div className="create-content"><main id="create-inputs" tabIndex={-1} aria-label="创作输入" className="create-inputs"><fieldset disabled={c.busy}><GeneratorPanel controller={c} /><PromptPanel controller={c} /><MediaInputPanel controller={c} /><ReferencePanel context={c.context} save={c.setReferences} busy={c.busy} /><ParameterPanel controller={c} /></fieldset></main><aside className="create-results"><RunStatusCard controller={c} /><CandidatePanel controller={c} route={props.route} /></aside></div><GenerateBar controller={c} /></>}
  </div>;
}
