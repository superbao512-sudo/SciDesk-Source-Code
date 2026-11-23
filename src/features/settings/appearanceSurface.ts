import type {AppSettings} from './settingsModel';

export const surfaceTokens={
 'appearance.page_surface_alpha':'--page-surface-alpha',
} as const;
export type SurfaceKey=keyof typeof surfaceTokens;
// Uncommitted slider drafts must survive unrelated preference events and slow
// earlier writes. The three shell surfaces share this single draft token.
const drafts=new Map<SurfaceKey,number>();
export function previewSurface(key:SurfaceKey,value:number){
 drafts.set(key,value);document.documentElement.style.setProperty(surfaceTokens[key],String(value));
}
export function applySurfaces(config:AppSettings){
 for(const key of Object.keys(surfaceTokens) as SurfaceKey[])document.documentElement.style.setProperty(surfaceTokens[key],String(drafts.get(key)??config[key]));
}
export function finishSurfacePreview(key:SurfaceKey,value:number,persisted:number){
 if(drafts.get(key)!==value)return;
 drafts.delete(key);document.documentElement.style.setProperty(surfaceTokens[key],String(persisted));
}
