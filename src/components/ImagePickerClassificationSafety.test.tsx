// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ImageAssetPicker } from "./ImageAssetPicker";


const api = vi.hoisted(() => ({
  list: vi.fn(),
  tagsList: vi.fn(),
  imageGet: vi.fn(),
}));
vi.mock("../product/client", () => ({ productClient: { library: api } }));

beforeEach(() => {
  vi.resetAllMocks();
  (URL as unknown as { createObjectURL: () => string }).createObjectURL = () => "blob:img";
  api.tagsList.mockResolvedValue([{ id: "t-char", name: "角色" }]);
  api.list.mockResolvedValue({
    items: [
      { resourceRef: { kind: "asset", id: "a1" }, title: "角色甲", subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: true },
      { resourceRef: { kind: "asset", id: "a2" }, title: "风景", subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: true },
    ],
    nextCursor: null,
    coverage: "complete-category",
    coverageMessage: "",
  });
  api.imageGet.mockResolvedValue([137, 80, 78, 71]);
});
afterEach(() => cleanup());


it('AUDIT-F07 officially character-tagged numeric names must not appear as unclassified',async()=>{
 const row={resourceRef:{kind:'asset',id:'numeric-character'},tags:[{id:'t-char',name:'角色'}],title:'55',subtype:'image',createdAt:'',updatedAt:'',thumbnailAvailable:true};
 // Server's character-tag query proves association; all-page returns the same row.
 api.tagsList.mockResolvedValue([{id:'t-char',name:'角色'}]);api.list.mockImplementation(async(_project,query)=>({items:query.tagId===null||query.tagId==='t-char'?[row]:[],nextCursor:null,coverage:'complete-category',coverageMessage:''}));
 render(<ImageAssetPicker projectId='project-a' open onSelect={vi.fn()} onClose={vi.fn()}/>);
 await screen.findByRole('option',{name:/55/});fireEvent.click(screen.getByRole('tab',{name:'角色'}));await waitFor(()=>expect(api.list).toHaveBeenCalledWith('project-a',expect.objectContaining({tagId:'t-char'})));await screen.findByRole('option',{name:/55/});
 const before=api.list.mock.calls.length;fireEvent.click(screen.getByRole('tab',{name:'未分类'}));await waitFor(()=>expect(api.list.mock.calls.length).toBeGreaterThan(before));await waitFor(()=>expect(screen.queryByText('正在加载图片…')).toBeNull());
 console.log('F07 '+JSON.stringify({serverCharacterTagConfirmed:true,shownAsUntagged:!!screen.queryByRole('option',{name:/55/})}));expect(screen.queryByRole('option',{name:/55/})).toBeNull();
});
it('CONTROL all tab retains selectable numeric filenames',async()=>{
 api.tagsList.mockResolvedValue([]);api.list.mockResolvedValue({items:[{resourceRef:{kind:'asset',id:'a55'},title:'55',subtype:'image',createdAt:'',updatedAt:'',thumbnailAvailable:true}],nextCursor:null,coverage:'complete-category',coverageMessage:''});render(<ImageAssetPicker projectId='p' open onSelect={vi.fn()} onClose={vi.fn()}/>);await screen.findByRole('option',{name:/55/});
});
it('AUDIT-F10 Escape must close an aria-modal image picker just like the Close button',async()=>{
 const close=vi.fn();render(<ImageAssetPicker projectId='p' open onSelect={vi.fn()} onClose={close}/>);const dialog=await screen.findByRole('dialog');
 fireEvent.keyDown(dialog,{key:'Escape',code:'Escape'});const afterEscape=close.mock.calls.length;
 fireEvent.click(screen.getByRole('button',{name:'关闭'}));console.log('F10 '+JSON.stringify({afterEscape,afterCloseButton:close.mock.calls.length}));expect(close).toHaveBeenCalledTimes(2);expect(afterEscape).toBe(1);
});
