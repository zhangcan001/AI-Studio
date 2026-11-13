// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { MediaLightboxScope, useMediaLightbox } from './CreateMediaPreview';
const api=vi.hoisted(()=>({imageGet:vi.fn()}));
vi.mock('../../product/client',()=>({productClient:{library:api,creation:{mediaUrl:()=>''}}}));
beforeEach(()=>{vi.resetAllMocks();URL.createObjectURL=vi.fn(()=>'blob:owned-audit');URL.revokeObjectURL=vi.fn();});
afterEach(()=>cleanup());
function Open({id}:{id:string}){const lightbox=useMediaLightbox();return <button onClick={()=>lightbox?.open({kind:'image',projectId:'audit-p',assetId:id,name:'audit-image'})}>打开测试图片</button>}
it('AUDIT-F09 full image read failure must end loading and show a recoverable state',async()=>{
 api.imageGet.mockRejectedValue({code:'LIBRARY_RESOURCE_NOT_FOUND',message:'fixture missing image'});
 render(<MediaLightboxScope><Open id='missing-image-audit'/></MediaLightboxScope>);fireEvent.click(screen.getByRole('button',{name:'打开测试图片'}));await screen.findByRole('dialog',{name:'预览 audit-image'});await waitFor(()=>expect(api.imageGet).toHaveBeenCalledTimes(1));await act(async()=>{});
 console.log('F09 '+JSON.stringify({imageReadRejected:true,loadingStillVisible:!!screen.queryByText('正在读取图片…')}));
 await waitFor(()=>expect(screen.queryByText('正在读取图片…')).toBeNull());expect(screen.getByText(/不可用|失败|重试/)).toBeTruthy();
});
it('CONTROL valid image response displays an image instead of indefinite loading',async()=>{
 api.imageGet.mockResolvedValue([137,80,78,71]);render(<MediaLightboxScope><Open id='good-image-audit'/></MediaLightboxScope>);fireEvent.click(screen.getByRole('button',{name:'打开测试图片'}));await screen.findByRole('img',{name:'audit-image'});expect(screen.queryByText('正在读取图片…')).toBeNull();
});

it('image preview recovers after explicit retry and Escape restores opener focus',async()=>{
 api.imageGet.mockRejectedValueOnce(Error('temporary')).mockResolvedValueOnce([137,80,78,71]);
 render(<MediaLightboxScope><Open id='retry-owned-image'/></MediaLightboxScope>);
 const opener=screen.getByRole('button',{name:'打开测试图片'});opener.focus();fireEvent.click(opener);
 fireEvent.click(await screen.findByRole('button',{name:'重试读取'}));await screen.findByRole('img',{name:'audit-image'});
 expect(api.imageGet).toHaveBeenCalledTimes(2);
 fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});expect(screen.queryByRole('dialog')).toBeNull();expect(document.activeElement).toBe(opener);
});
