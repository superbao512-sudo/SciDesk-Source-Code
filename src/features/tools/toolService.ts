import {isTauri} from '@tauri-apps/api/core';
import {projectCommand} from '../projects/projectService';
export type LocalToolInfo={root:string;id:string;version:string;runtime:'exe'|'html';entry:string;available:boolean};
export type ToolEntry={id:string;name:string;description:string;url:string;icon:string|null;icon_warning:string|null;created_at:string;updated_at:string;kind:'website'|'local';local:LocalToolInfo|null};
export type LocalToolManifest={schema_version:1;id:string;name:string;description:string;version:string;runtime:'exe'|'html';entry:string;args:string[];icon:string|null};
export type LocalToolPreview={root:string;manifest:LocalToolManifest;icon:string|null;token:string;existing_id:string|null};
export type ToolInput={id?:string;name:string;description:string;url:string;icon?:string|null};
export const toolService={
 list:()=>projectCommand<ToolEntry[]>('tools_list'),
 save:(input:ToolInput)=>projectCommand<{id:string}>('tools_save',input),
 remove:(id:string)=>projectCommand<boolean>('tools_remove',{id}),
 previewLocal:(root:string)=>projectCommand<LocalToolPreview>('tools_preview_local',{root}),
 registerLocal:(root:string,token:string)=>projectCommand<{id:string}>('tools_register_local',{root,token}),
 pickLocal:()=>projectCommand<string|null>('tools_pick_local'),
};
export function validToolUrl(raw:string){try{const url=new URL(raw);return raw.length<=8192&&!Array.from(raw).some(c=>/\s/.test(c)||c.charCodeAt(0)<32||c.charCodeAt(0)===127)&&['http:','https:'].includes(url.protocol)&&!!url.hostname&&!url.username&&!url.password}catch{return false}}
// Every website tool uses the same registration model and launcher.
export async function launchTool(tool:Pick<ToolEntry,'id'|'url'>&{kind?:ToolEntry['kind']}){
 if(tool.kind==='local'){await projectCommand('tools_open',{id:tool.id});return}
 if(!validToolUrl(tool.url))throw new Error('工具网址无效，请先编辑');
 if(isTauri())await projectCommand('tools_open',{id:tool.id});
 else window.open(tool.url,'_blank','noopener,noreferrer');
}
export async function readToolIcon(file:File):Promise<string>{
 if(file.size>2*1024*1024||!['image/png','image/jpeg','image/webp'].includes(file.type))throw new Error('请选择不超过 2 MB 的 PNG、JPEG 或 WebP 图片');
 const raw=await new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(new Error('图标读取失败'));reader.readAsDataURL(file)});
 const image=await new Promise<HTMLImageElement>((resolve,reject)=>{const image=new Image();image.onload=()=>resolve(image);image.onerror=()=>reject(new Error('图片无法解码，请重新选择'));image.src=raw});
 const canvas=document.createElement('canvas');const scale=Math.min(1,256/Math.max(image.naturalWidth,image.naturalHeight));canvas.width=Math.max(1,Math.round(image.naturalWidth*scale));canvas.height=Math.max(1,Math.round(image.naturalHeight*scale));const context=canvas.getContext('2d');if(!context)throw new Error('图标处理失败');context.drawImage(image,0,0,canvas.width,canvas.height);return canvas.toDataURL('image/png');
}
