// @vitest-environment jsdom
import {cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
// @ts-expect-error Node-only source guard.
import {readFileSync} from 'node:fs';
import {AppShellV3} from './v3/AppShellV3';
import {projectDisplayName} from '../i18n/statusLabels';
import {preservesCreateDraftForSettings} from './App';
afterEach(cleanup);
it('M2-2 preserves only owned Create to Library draft navigation',()=>{
 const route={kind:'create',projectId:'p',shotId:'s',stage:'video'} as const;
 expect(preservesCreateDraftForSettings(route,{kind:'library',projectId:'p',filter:'prompts'},'p:s:video')).toBe(true);
 expect(preservesCreateDraftForSettings(route,{kind:'library',projectId:'other'},'p:s:video')).toBe(false);
 expect(preservesCreateDraftForSettings(route,{kind:'library',projectId:'p'},undefined)).toBe(false);
});
it.each([['prj_default','Default Project','默认项目'],['prj_owned','My Film','My Film']])('M2-2 Shell displays %s through the existing name helper',(id,name,label)=>{
 render(<AppShellV3 route={{kind:'project',projectId:id,page:'overview'}} projectName={projectDisplayName(id,name)} navigate={vi.fn()} back={vi.fn()} projectSelector={<select aria-label="当前项目"><option>{label}</option></select>}><p>owned</p></AppShellV3>);
 expect(screen.getAllByText(label).length).toBeGreaterThan(0);
 expect(readFileSync('src/app/App.tsx','utf8')).toContain('projectName={activeProject ? projectDisplayName(activeProject.id, activeProject.name) : undefined}');
});
