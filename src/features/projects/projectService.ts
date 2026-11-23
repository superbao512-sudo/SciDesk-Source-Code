import {invoke,isTauri} from '@tauri-apps/api/core';
export interface LocalProject {id:string;name:string;description:string;root_path:string;icon_name:string;project_color:string;status:'active'|'completed'|'idea';progress:number;progress_enabled:boolean;sort_order:number;source_type:'created'|'imported';created_at:string;updated_at:string}
export interface PendingOperation {id:string;kind:string;old_path:string;new_path:string;fs_done:boolean}
export interface ProjectState {projects:LocalProject[];selected_id:string|null;default_root:string;pending:PendingOperation[]}
export interface FileEntry {name:string;is_dir:boolean;is_link:boolean;size:number|null;modified:number|null;extension:string}
export interface Directory {path:string;relative:string;entries:FileEntry[]}
export interface TemplateResult {created:string[];skipped:string[];failed:{name:string;error:string}[]}
export const statusLabels={active:'进行中',completed:'已结束',idea:'调研中'} as const;
async function send<T>(op:string,args:Record<string,unknown>):Promise<T>{
 if(isTauri())return invoke<T>('project_command',{op,args});
 const response=await fetch('/__scidesk/projects',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({op,args})});
 const result=await response.json();if(!response.ok||!result.ok)throw new Error(result.error||'本地项目服务请求失败');return result.value as T;
}
// Keep writes in user-action order, including across route changes. Reads remain cancellable in the view.
let writeQueue:Promise<void>=Promise.resolve();
export function projectCommand<T>(op:string,args:Record<string,unknown>={}):Promise<T>{
 if(['bootstrap','list_files','pick_directory','open','reveal','manual_list','manual_browse','manual_open','manual_pick_folder','manual_pick_file'].includes(op))return send<T>(op,args);
 const result=writeQueue.then(()=>send<T>(op,args));writeQueue=result.then(()=>{},()=>{});return result;
}
export const pickDirectory=(start='')=>projectCommand<string|null>('pick_directory',{start});
export const joinRelative=(parent:string,name:string)=>parent?`${parent}/${name}`:name;
export const joinDisplay=(parent:string,name:string)=>`${parent.replace(/[\\/]+$/,'')}\\${name.replaceAll('/','\\')}`;
