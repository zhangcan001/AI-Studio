// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StoryEntityCategorySection } from "./StoryEntityCategorySection";
import type { AssetView } from "../../types/asset";
import { STORY_ENTITIES_CHANGED, type StoryEntityKind } from "../../types/storyEntity";
import { useStudioStore } from "../../stores/studioStore";

const pickerMock = vi.hoisted(() => ({
  onImportFile: undefined as (() => void) | undefined,
  onSelect: undefined as ((id: string) => void) | undefined,
}));
const api = vi.hoisted(() => ({ list: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), imageGet: vi.fn(), thumbnailGet: vi.fn(), importImages: vi.fn() }));
vi.mock("../../product/client", () => ({
  productClient: {
    storyEntities: { list: api.list, create: api.create, update: api.update, delete: api.delete, importImages: api.importImages },
    library: { imageGet: api.imageGet, thumbnailGet: api.thumbnailGet },
  },
}));
vi.mock("../../components/ImageAssetPicker", () => ({
  ImageAssetPicker: (props: { open?: boolean; onImportFile?: () => void; onSelect?: (id: string) => void }) => {
    pickerMock.onImportFile = props.open ? props.onImportFile : undefined;
    pickerMock.onSelect = props.open ? props.onSelect : undefined;
    return props.open ? <div data-testid="mock-picker" /> : null;
  },
}));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("does not rename over unsaved Shot drafts and does not discard them",async()=>{
 const entity={id:"entity",projectId:"p",kind:"character",name:"Ann",description:"",coverAssetId:null,imageAssetIds:[],createdAt:"",updatedAt:""};
 api.list.mockResolvedValue([entity]);const key=JSON.stringify(["p","shot","recipe"]);
 useStudioStore.setState({storyboardPrompts:{[key]:"@Ann runs"},storyboardFields:{},draftDirty:false});
 try {
   render(<StoryEntityCategorySection projectId="p" kind="character"/>);
   fireEvent.click(await screen.findByRole("button",{name:"编辑"}));
   fireEvent.change(screen.getByLabelText("角色名称"),{target:{value:"Ada"}});
   fireEvent.click(screen.getByRole("button",{name:"保存"}));
   expect(screen.getByRole("alert").textContent).toContain("请先保存或放弃");
   expect(api.update).not.toHaveBeenCalled();expect(useStudioStore.getState().storyboardPrompts[key]).toBe("@Ann runs");
 } finally {useStudioStore.setState({storyboardPrompts:{},storyboardFields:{},draftDirty:false});}
});

it.each([
  { kind: "character" as StoryEntityKind, label: "角色", filename: "苏蔓.png" },
  { kind: "scene" as StoryEntityKind, label: "场景", filename: "夜晚街道.webp" },
  { kind: "prop" as StoryEntityKind, label: "道具", filename: "古剑.jpg" },
  { kind: "composition" as StoryEntityKind, label: "构图", filename: "剧情01.png" },
])("bulk imports $label images as separate entities named from their filenames", async ({ kind, label, filename }) => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({
    imported: [{ id: "asset-1", name: filename, originalName: filename } as AssetView],
    failed: [],
    cancelled: false,
  });
  api.create.mockResolvedValue({});

  render(<StoryEntityCategorySection projectId="project-a" kind={kind} />);
  const bulkButton = await screen.findByRole("button", { name: `批量导入${label}图片` });
  await waitFor(() => expect(bulkButton.hasAttribute("disabled")).toBe(false));
  fireEvent.click(bulkButton);

  await waitFor(() => expect(api.create).toHaveBeenCalledWith({
    projectId: "project-a",
    kind,
    name: filename.slice(0, filename.lastIndexOf(".")),
    description: "",
    coverAssetId: "asset-1",
    imageAssetIds: ["asset-1"],
  }));
  expect(await screen.findByText(new RegExp(`批量导入完成：成功创建 1 个${label}`))).toBeTruthy();
  expect(api.list).toHaveBeenLastCalledWith("project-a", kind);
});

it("does not create entities when the image picker is cancelled", async () => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({ imported: [], failed: [], cancelled: true });
  render(<StoryEntityCategorySection projectId="project-a" kind="scene" />);
  await waitFor(() => expect(screen.getByRole("button", { name: "批量导入场景图片" }).hasAttribute("disabled")).toBe(false));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "批量导入场景图片" })); });
  expect(api.importImages).toHaveBeenCalledWith("project-a");
  expect(api.create).not.toHaveBeenCalled();
  expect(screen.queryByText(/批量导入完成/)).toBeNull();
});

it("reports partial failures without discarding successfully imported images", async () => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({
    imported: [
      { id: "asset-good", name: "古剑.jpg", originalName: "古剑.jpg" },
      { id: "asset-duplicate", name: "旧剑.png", originalName: "旧剑.png" },
    ],
    failed: [{ displayName: "损坏.png", error: "格式不合法" }], cancelled: false,
  });
  api.create.mockResolvedValueOnce({}).mockRejectedValueOnce(new Error("名称已存在"));
  render(<StoryEntityCategorySection projectId="project-a" kind="prop" />);
  await waitFor(() => expect(screen.getByRole("button", { name: "批量导入道具图片" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "批量导入道具图片" }));
  expect(await screen.findByText(/成功创建 1 个道具.*失败 2 项/)).toBeTruthy();
  expect(screen.getByText(/已导入的图片仍保留在媒体库/)).toBeTruthy();
  expect(api.create).toHaveBeenCalledTimes(2);
  expect(api.delete).not.toHaveBeenCalled();
});

it("ignores a late list response after the keyed project/category view changes", async () => {
  let resolveOld!: (value: unknown[]) => void;
  api.list.mockImplementation((project: string) => project === "project-a"
    ? new Promise(resolve => { resolveOld = resolve; })
    : Promise.resolve([{ id: "scene-b", projectId: "project-b", kind: "scene", name: "新场景", description: "", coverAssetId: null, imageAssetIds: [] }]));
  const view = render(<StoryEntityCategorySection key="a:character" projectId="project-a" kind="character" />);
  view.rerender(<StoryEntityCategorySection key="b:scene" projectId="project-b" kind="scene" />);
  expect(await screen.findByText("新场景")).toBeTruthy();
  await act(async () => { resolveOld([{ id: "old-character", name: "迟到的旧角色", description: "", coverAssetId: null, imageAssetIds: [] }]); });
  expect(screen.queryByText("迟到的旧角色")).toBeNull();
  expect(screen.getByRole("region", { name: "场景管理" })).toBeTruthy();
});

it("bounds entity thumbnail reads and cancels queued work on unmount", async () => {
  class VisibleObserver { constructor(private notify: (entries: {isIntersecting:boolean}[]) => void) {} observe() { this.notify([{isIntersecting:true}]); } disconnect() {} }
  vi.stubGlobal("IntersectionObserver", VisibleObserver);
  const releases: Array<() => void> = [];
  api.list.mockResolvedValue(Array.from({length:100},(_,i)=>({id:`e${i}`,projectId:"p",kind:"character",name:`E${i}`,description:"",coverAssetId:`a${i}`,imageAssetIds:[`a${i}`],createdAt:"",updatedAt:""})));
  api.thumbnailGet.mockImplementation(()=>new Promise(resolve=>releases.push(()=>resolve([1]))));
  const view=render(<StoryEntityCategorySection projectId="p" kind="character"/>);
  await screen.findByText("E99");await waitFor(()=>expect(api.thumbnailGet).toHaveBeenCalledTimes(4));
  expect(api.imageGet).not.toHaveBeenCalled();
  view.unmount();await act(async()=>{releases.forEach(release=>release());});
  expect(api.thumbnailGet).toHaveBeenCalledTimes(4);
  vi.unstubAllGlobals();
});

it("groups selected reference views only after explicit preview and save", async () => {
  api.list.mockResolvedValue([]);api.importImages.mockResolvedValue({imported:[{id:"front",name:"正面.png",originalName:"正面.png"},{id:"side",name:"侧面.png",originalName:"侧面.png"}],failed:[],cancelled:false});
  render(<StoryEntityCategorySection projectId="project-a" kind="character"/>);
  fireEvent.change(screen.getByRole("combobox",{name:"实体图片导入方式"}),{target:{value:"group"}});
  const button=await screen.findByRole("button",{name:"批量导入角色图片"});await waitFor(()=>expect(button.hasAttribute("disabled")).toBe(false));fireEvent.click(button);
  const name=await screen.findByRole("textbox",{name:"角色名称"});expect(api.create).not.toHaveBeenCalled();
  fireEvent.change(name,{target:{value:"同一角色"}});fireEvent.click(screen.getByRole("button",{name:"保存"}));
  await waitFor(()=>expect(api.create).toHaveBeenCalledWith({projectId:"project-a",kind:"character",name:"同一角色",description:"",coverAssetId:"front",imageAssetIds:["front","side"]}));
});

it("only guards actual entity edits and preserves the current editor when replacing it is declined",async()=>{
 const a={id:"entity-a",projectId:"p",kind:"character",name:"A",description:"",coverAssetId:null,imageAssetIds:[],createdAt:"",updatedAt:""};const b={...a,id:"entity-b",name:"B"};api.list.mockResolvedValue([a,b]);const dirty=vi.fn();const confirm=vi.spyOn(window,"confirm").mockReturnValue(false);
 render(<StoryEntityCategorySection projectId="p" kind="character" onDirtyChange={dirty}/>);await screen.findByText("A");fireEvent.click(screen.getAllByRole("button",{name:"编辑"})[0]);expect(dirty).not.toHaveBeenLastCalledWith(true);
 fireEvent.change(screen.getByLabelText("角色名称"),{target:{value:"A edited"}});await waitFor(()=>expect(dirty).toHaveBeenLastCalledWith(true));fireEvent.click(screen.getAllByRole("button",{name:"编辑"})[1]);expect(confirm).toHaveBeenCalledOnce();expect((screen.getByLabelText("角色名称") as HTMLInputElement).value).toBe("A edited");
});

it("shows non-blocking English prompt hint for Chinese entity description and allows 1-click cover switch", async () => {
  const entity = { id: "e-1", projectId: "p", kind: "character" as const, name: "Lin", description: "红衣长发剑客", coverAssetId: "img-1", imageAssetIds: ["img-1", "img-2"], createdAt: "", updatedAt: "" };
  api.list.mockResolvedValue([entity]);
  api.update.mockResolvedValue(entity);
  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  expect(screen.getByLabelText("描述语言建议").textContent).toContain("英文");
  fireEvent.click(screen.getByRole("button", { name: "将图片 img-2 设为封面" }));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(expect.objectContaining({ id: "e-1", coverAssetId: "img-2" })));
});

it("ignores late importImages when switching project/kind and surfaces picker onImportFile failures", async () => {
  let resolveOldImport!: (val: { imported: AssetView[]; failed: { displayName: string; error: string }[]; cancelled: boolean }) => void;
  api.list.mockResolvedValue([]);
  api.importImages.mockImplementationOnce(() => new Promise(resolve => { resolveOldImport = resolve; }));
  const view = render(<StoryEntityCategorySection projectId="project-a" kind="character" />);
  const bulkBtn = await screen.findByRole("button", { name: "批量导入角色图片" });
  await waitFor(() => expect(bulkBtn.hasAttribute("disabled")).toBe(false));
  fireEvent.click(bulkBtn);
  view.rerender(<StoryEntityCategorySection projectId="project-b" kind="scene" />);
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("project-b", "scene"));
  await act(async () => {
    resolveOldImport({ imported: [{ id: "old-asset", name: "old.png", originalName: "old.png" } as AssetView], failed: [], cancelled: false });
  });
  expect(api.create).not.toHaveBeenCalled();

  api.importImages.mockResolvedValueOnce({ imported: [], failed: [{ displayName: "bad.png", error: "损坏的图片" }], cancelled: false });
  fireEvent.click(screen.getByRole("button", { name: "新建场景" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  expect(pickerMock.onImportFile).toBeTypeOf("function");
  await act(async () => { pickerMock.onImportFile?.(); });
  const dialog = screen.getByRole("dialog", { name: "新建场景" });
  expect(within(dialog).getByRole("alert").textContent).toContain("bad.png：损坏的图片");
});

it("still dispatches STORY_ENTITIES_CHANGED when unmounted immediately after saving", async () => {
  const entity = { id: "e-save", projectId: "p", kind: "character" as const, name: "林风", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
  api.list.mockResolvedValue([entity]);
  let resolveUpdate!: () => void;
  api.update.mockImplementationOnce(() => new Promise<void>(resolve => { resolveUpdate = resolve; }));
  const listener = vi.fn();
  window.addEventListener(STORY_ENTITIES_CHANGED, listener);
  try {
    const view = render(<StoryEntityCategorySection projectId="p" kind="character" />);
    fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
    fireEvent.change(screen.getByLabelText("角色名称"), { target: { value: "林风新版" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(api.update).toHaveBeenCalledTimes(1));
    view.unmount();
    await act(async () => { resolveUpdate(); });
    expect(listener).toHaveBeenCalledTimes(1);
  } finally {
    window.removeEventListener(STORY_ENTITIES_CHANGED, listener);
  }
});

it("does not let an earlier cover import overwrite a newer explicit cover selection", async () => {
  const entity = { id: "e-cover", projectId: "p", kind: "character" as const, name: "Lin", description: "", coverAssetId: "img-1", imageAssetIds: ["img-1", "img-2"], createdAt: "", updatedAt: "" };
  api.list.mockResolvedValue([entity]);
  api.update.mockResolvedValue(entity);
  let resolveImport!: (val: { imported: AssetView[]; failed: { displayName: string; error: string }[]; cancelled: boolean }) => void;
  api.importImages.mockImplementationOnce(() => new Promise(resolve => { resolveImport = resolve; }));

  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  act(() => { pickerMock.onImportFile?.(); });
  await waitFor(() => expect(api.importImages).toHaveBeenCalledTimes(1));

  fireEvent.click(screen.getByRole("button", { name: "将图片 img-2 设为封面" }));
  await act(async () => {
    resolveImport({
      imported: [{ id: "late-cover", name: "late.png", originalName: "late.png" } as AssetView],
      failed: [],
      cancelled: false,
    });
  });

  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(expect.objectContaining({ id: "e-cover", coverAssetId: "img-2" })));
});

it("keeps import lock isolated per import operation across project switches", async () => {
  const eA = { id: "e-a", projectId: "p-a", kind: "character" as const, name: "甲", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
  const eB = { id: "e-b", projectId: "p-b", kind: "character" as const, name: "乙", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
  api.list.mockImplementation(async (projectId: string) => (projectId === "p-a" ? [eA] : [eB]));
  type ImportResult = { imported: AssetView[]; failed: { displayName: string; error: string }[]; cancelled: boolean };
  let resolveA!: (v: ImportResult) => void;
  let resolveB!: (v: ImportResult) => void;
  api.importImages
    .mockImplementationOnce(() => new Promise<ImportResult>(resolve => { resolveA = resolve; }))
    .mockImplementationOnce(() => new Promise<ImportResult>(resolve => { resolveB = resolve; }));

  const { rerender } = render(<StoryEntityCategorySection projectId="p-a" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  act(() => { pickerMock.onImportFile?.(); });
  await waitFor(() => expect(api.importImages).toHaveBeenCalledWith("p-a"));

  rerender(<StoryEntityCategorySection projectId="p-b" kind="character" />);
  await screen.findByText("乙");
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  act(() => { pickerMock.onImportFile?.(); });
  await waitFor(() => expect(api.importImages).toHaveBeenCalledWith("p-b"));

  await act(async () => { resolveA({ imported: [], failed: [], cancelled: true }); });
  act(() => { pickerMock.onImportFile?.(); });
  expect(api.importImages).toHaveBeenCalledTimes(2);

  await act(async () => { resolveB({ imported: [], failed: [], cancelled: true }); });
});

it("clears previous notice when opening another entity editor", async () => {
  const e1 = { id: "e-1", projectId: "p", kind: "character" as const, name: "角色一", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
  const e2 = { id: "e-2", projectId: "p", kind: "character" as const, name: "角色二", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
  api.list.mockResolvedValue([e1, e2]);
  api.update.mockResolvedValueOnce({ ...e1, name: "角色一新" });

  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  const editButtons = await screen.findAllByRole("button", { name: "编辑" });
  fireEvent.click(editButtons[0]);
  fireEvent.change(screen.getByLabelText("角色名称"), { target: { value: "角色一新" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText(/已更新角色「角色一新」/);

  const refreshedEdits = screen.getAllByRole("button", { name: "编辑" });
  fireEvent.click(refreshedEdits[1]);
  const dialog = screen.getByRole("dialog", { name: "编辑角色" });
  expect(within(dialog).queryByRole("status")).toBeNull();
  expect(screen.queryByText(/已更新角色「角色一新」/)).toBeNull();
});

it("locks form inputs and image controls while save is in flight so edits are not lost", async () => {
  const entity = {
    id: "e-lock",
    projectId: "p",
    kind: "character" as const,
    name: "初版名称",
    description: "初版描述",
    coverAssetId: "img-1",
    imageAssetIds: ["img-1", "img-2"],
    createdAt: "",
    updatedAt: "",
  };
  api.list.mockResolvedValue([entity]);
  let resolveUpdate!: () => void;
  api.update.mockImplementationOnce(() => new Promise<void>(resolve => { resolveUpdate = resolve; }));

  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  const nameInput = screen.getByLabelText("角色名称") as HTMLInputElement;
  const descInput = screen.getByLabelText("角色描述") as HTMLTextAreaElement;
  fireEvent.change(nameInput, { target: { value: "第一版" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(api.update).toHaveBeenCalledTimes(1));
  expect(nameInput.disabled).toBe(true);
  expect(descInput.disabled).toBe(true);
  expect((screen.getByRole("button", { name: "选择封面" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "清除" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "将图片 img-2 设为封面" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "移除图片 img-1" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "添加图片" }) as HTMLButtonElement).disabled).toBe(true);

  await act(async () => { resolveUpdate(); });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "编辑角色" })).toBeNull());
  expect(api.update).toHaveBeenCalledWith(expect.objectContaining({ id: "e-lock", name: "第一版" }));
});

it("disables new picker file import while an import is already in flight and supports Ctrl+Enter save", async () => {
  const entity = {
    id: "e-hotkey",
    projectId: "p",
    kind: "character" as const,
    name: "捷径",
    description: "",
    coverAssetId: "img-1",
    imageAssetIds: ["img-1", "img-2"],
    createdAt: "",
    updatedAt: "",
  };
  api.list.mockResolvedValue([entity]);
  api.update.mockResolvedValue({ ...entity, name: "捷径新版" });
  let resolveImport!: (val: { imported: AssetView[]; failed: { displayName: string; error: string }[]; cancelled: boolean }) => void;
  api.importImages.mockImplementationOnce(() => new Promise(resolve => { resolveImport = resolve; }));

  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  expect(pickerMock.onImportFile).toBeTypeOf("function");

  act(() => { pickerMock.onImportFile?.(); });
  await waitFor(() => expect(api.importImages).toHaveBeenCalledTimes(1));

  // Existing controls like "将图片 img-2 设为封面" remain usable, while picker omits onImportFile when reopened during busy import
  expect((screen.getByRole("button", { name: "将图片 img-2 设为封面" }) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  expect(pickerMock.onImportFile).toBeUndefined();

  await act(async () => { resolveImport({ imported: [], failed: [], cancelled: true }); });
  await waitFor(() => expect(pickerMock.onImportFile).toBeTypeOf("function"));

  // Close mock picker via onSelect, then trigger Ctrl+Enter save
  act(() => { pickerMock.onSelect?.("img-2"); });
  fireEvent.change(screen.getByLabelText("角色名称"), { target: { value: "捷径新版" } });
  fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(expect.objectContaining({ id: "e-hotkey", name: "捷径新版", coverAssetId: "img-2" })));
});

it("adds all imported images from multi-file cover import to associated images and explains it accurately", async () => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({
    imported: [
      { id: "cov-1", name: "封面图.png", originalName: "封面图.png" } as AssetView,
      { id: "cov-2", name: "侧视图.png", originalName: "侧视图.png" } as AssetView,
    ],
    failed: [],
    cancelled: false,
  });
  api.create.mockResolvedValue({});
  render(<StoryEntityCategorySection projectId="p" kind="character" />);
  fireEvent.click(await screen.findByRole("button", { name: "新建角色" }));
  fireEvent.click(screen.getByRole("button", { name: "选择封面" }));
  await act(async () => { pickerMock.onImportFile?.(); });

  expect(await screen.findByText(/已从电脑导入 2 张图片，已将第 1 张设为封面并全部加入关联图片/)).toBeTruthy();
  expect(screen.getByText("关联图片（2）")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("角色名称"), { target: { value: "多图封面角色" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.create).toHaveBeenCalledWith(expect.objectContaining({
    name: "多图封面角色",
    coverAssetId: "cov-1",
    imageAssetIds: ["cov-1", "cov-2"],
  })));
});
