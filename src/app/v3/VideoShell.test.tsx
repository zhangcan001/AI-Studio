// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AppShellV3 } from "./AppShellV3";
import { RuntimeConnectionIndicator } from "./ShellChrome";
import { overviewAction } from "./ProjectOverviewPage";
import { fromLegacyLocation, toLegacyLocation } from "../routes/legacyAdapter";
import { parseRoute } from "../routes/resumeAdapter";
import type { AppRoute } from "../routes/types";
import type { ComfyStatus } from "../../types/comfy";
import { useAppRoute } from "../routes/useAppRoute";
import { useDraftConfirmation } from "../routes/useDraftConfirmation";
// @ts-expect-error Node-only style assertions supplement Native screenshots.
import { readFileSync } from "node:fs";
afterEach(() => { cleanup(); localStorage.clear(); vi.restoreAllMocks(); });
const create = { kind:"create",projectId:"p",shotId:"s",stage:"video" } as const;
function shell(route: AppRoute = create, navigate = vi.fn()) {
  const view = render(<AppShellV3 route={route} projectName="很长的中文项目名称" projectSelector={<select aria-label="当前项目"><option>项目</option></select>} navigate={navigate} back={vi.fn()}><p>真实子页面</p></AppShellV3>);
  return { ...view, navigate };
}
it("keeps five visible named primary actions, exact active entry, and video default", () => {
  const { navigate } = shell();
  const nav = within(screen.getByRole("navigation",{name:"项目导航"}));
  expect(nav.getAllByRole("button").map(b=>b.getAttribute("aria-label"))).toEqual(["概览","创作","运行","素材库","设置"]);
  fireEvent.click(nav.getByRole("button",{name:"创作"}));
  expect(navigate).toHaveBeenLastCalledWith({kind:"create",projectId:"p",stage:"video"});
  fireEvent.keyDown(window,{ctrlKey:true,key:"k"});
  expect(navigate).toHaveBeenLastCalledWith({kind:"create",projectId:"p",stage:"video"});
  expect(nav.getByRole("button",{name:"创作"}).getAttribute("aria-current")).toBe("page");
  expect(nav.getByRole("button",{name:"设置"}).hasAttribute("aria-current")).toBe(false);
});
it("Settings carries the exact return context and advanced/project entries remain real", () => {
  const { navigate, rerender } = shell();
  fireEvent.click(screen.getByRole("button",{name:"设置"}));
  const settings = {kind:"system-settings",section:"general",returnTo:create} as const;
  expect(navigate).toHaveBeenLastCalledWith(settings);
  rerender(<AppShellV3 route={settings} projectSelector={null} navigate={navigate} back={vi.fn()}>设置内容</AppShellV3>);
  expect(screen.getByRole("button",{name:"设置"}).getAttribute("aria-current")).toBe("page");
  expect(screen.getByRole("button",{name:"概览"}).hasAttribute("aria-current")).toBe(false);
  fireEvent.click(screen.getByRole("button",{name:"高级工作流"}));
  expect(navigate).toHaveBeenLastCalledWith({kind:"system-settings",section:"advanced-workflows",returnTo:create});
  fireEvent.click(screen.getByRole("button",{name:"项目设置"}));
  expect(navigate).toHaveBeenLastCalledWith({kind:"project-settings",projectId:"p",section:"general"});
});
it("keeps project selection and collapse independently operable without another route owner", () => {
  shell();
  expect(screen.getByRole("combobox",{name:"当前项目"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"折叠导航"}));
  expect(screen.getByRole("button",{name:"展开导航"}).getAttribute("aria-expanded")).toBe("false");
  for (const name of ["概览","创作","运行","素材库","设置"]) expect(screen.getByRole("button",{name})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"展开导航"}));
  expect(screen.getByRole("button",{name:"折叠导航"}).getAttribute("aria-expanded")).toBe("true");
  expect(localStorage.length).toBe(0);
});
it.each([
  [undefined,"状态未检查"], [{status:"OFFLINE"},"未连接"],
  [{status:"CONNECTED"},"已连接 · 能力未确认"],
  [{status:"CONNECTED",capability:{nodeCount:1,capturedAt:""}},"已连接 · 能力已检查"],
])("projects cached runtime facts honestly: %j", (patch,label) => {
  const runtime = patch ? {endpoint:"",devices:[],...patch} as ComfyStatus : undefined;
  render(<RuntimeConnectionIndicator runtime={runtime}/>);
  expect(screen.getByRole("status").textContent).toContain(label);
  expect(screen.queryByText(/全部.*就绪|正在生成|积分/)).toBeNull();
});
it("defaults unspecified ordinary legacy Shot to video while preserving explicit IMAGE and batch history", () => {
  expect(fromLegacyLocation({projectId:"p",workspace:"shots",shotId:"s"})).toEqual(create);
  const image = {...create,stage:"image"} as const;
  expect(fromLegacyLocation({projectId:"p",workspace:"shots",shotId:"s",stage:"IMAGE"})).toEqual(image);
  expect(parseRoute(image)).toEqual({...image,surface:undefined});
  expect(toLegacyLocation(image)).toMatchObject({projectId:"p",shotId:"s",stage:"IMAGE"});
  expect(fromLegacyLocation({projectId:"p",workspace:"studio"})).toEqual({kind:"create",projectId:"p",stage:"image",surface:"batch"});
  const action = {kind:"IMAGE_REVIEW",shotId:"s",taskId:null,batchId:null,assetId:null,priority:1,reasonCode:"",reason:""};
  expect(overviewAction("p",action).route).toEqual(image);
  expect(overviewAction("p",{...action,kind:"READY"}).route).toEqual(create);
});
it("Ctrl+K awaits the existing explicit dirty-draft decision before changing the canonical route", async () => {
  const show = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype,"showModal");
  const close = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype,"close");
  Object.defineProperty(HTMLDialogElement.prototype,"showModal",{configurable:true,value(){this.open=true;}});
  Object.defineProperty(HTMLDialogElement.prototype,"close",{configurable:true,value(){this.open=false;}});
  function Host() {
    const r = useAppRoute(); const d = useDraftConfirmation();
    const navigate = async (route:AppRoute) => { if (await d.confirm("未保存的镜头草稿")) r.navigate(route); };
    return <>{d.dialog}<AppShellV3 route={r.route} projectSelector={null} navigate={navigate} back={r.back}><button onClick={()=>r.restore({kind:"project",projectId:"p",page:"overview"})}>载入项目</button><output>{r.route.kind}</output></AppShellV3></>;
  }
  try {
  render(<Host/>); fireEvent.click(screen.getByRole("button",{name:"载入项目"}));
  fireEvent.keyDown(window,{ctrlKey:true,key:"k"});
  expect(screen.getByRole("dialog").textContent).toContain("未保存");
  fireEvent.click(screen.getByRole("button",{name:"继续编辑"}));
  await act(async()=>{});
  expect(screen.getByText("project",{selector:"output"})).toBeTruthy();
  fireEvent.keyDown(window,{ctrlKey:true,key:"k"});
  fireEvent.click(screen.getByRole("button",{name:"放弃修改并继续"}));
  await act(async()=>{});
  expect(screen.getByText("create",{selector:"output"})).toBeTruthy();
  } finally {
    cleanup();
    if(show) Object.defineProperty(HTMLDialogElement.prototype,"showModal",show); else Reflect.deleteProperty(HTMLDialogElement.prototype,"showModal");
    if(close) Object.defineProperty(HTMLDialogElement.prototype,"close",close); else Reflect.deleteProperty(HTMLDialogElement.prototype,"close");
  }
});
it("retains responsive zero-min grids, tokens, focus and reduced motion rather than hiding layout overflow", () => {
  const css = readFileSync("src/app/v3/AppShellV3.css","utf8");
  expect(css).toContain("minmax(0,1fr)");
  expect(css).toContain("min-width:0");
  expect(css).toContain("@media (max-width:1200px)");
  expect(css).toContain("@media (max-width:760px)");
  expect(css).toContain("prefers-reduced-motion:reduce");
  expect(css).toContain(":focus-visible");
  // The runtime banner and full-height child must share the available height;
  // otherwise the Generate footer is pushed below the default Native window.
  expect(css).toContain("display:flex; flex-direction:column; flex:1; min-height:0; overflow:hidden;");
  expect(css).toContain(".v3-main .create-page { flex:1; min-height:0; height:auto; }");
  expect(css).not.toMatch(/!important|#[\da-f]{3,8}\b/i);
  for (const path of ["src/features/create/CreatePage.css", "src/features/library/LibraryPage.css", "src/features/runs/RunsPage.css"]) {
    expect(readFileSync(path,"utf8")).not.toMatch(/\.v3-shell:has|\.v3-layout:has/);
  }
});
