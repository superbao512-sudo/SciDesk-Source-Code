import {invoke,isTauri} from '@tauri-apps/api/core';
import {IdeasClient,attachmentBlob,type Attachment} from '../../../public/ideas-mobile/core.js';
export {IdeasClient,uid,emptyIdea,matching,type Idea,type CanvasState,type Action,type Attachment} from '../../../public/ideas-mobile/core.js';
export async function ideaCommand<T=unknown>(op:string,args:Record<string,unknown>={}):Promise<T>{
 if(isTauri())return invoke<T>('ideas_command',{op,args});
 const response=await fetch('/__scidesk/ideas',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({op,args})});
 let result;try{result=await response.json()}catch{throw new Error('想法服务响应无效，请检查本地开发服务')}
 if(!response.ok||!result.ok)throw result.error||new Error('想法服务请求失败');return result.value as T;
}
let opening:Promise<IdeasClient>|undefined;
export function getIdeasClient(){opening??=new IdeasClient(isTauri()?'scidesk-ideas-desktop-v1':'scidesk-ideas-browser-v1',ideaCommand).open().catch(error=>{opening=undefined;throw error});return opening;}
export function message(error:unknown){return error&&typeof error==='object'&&'message' in error?String(error.message):String(error)}
interface FileData{base64:string;media_type:string;name:string}
export async function attachmentData(client:IdeasClient,id:string):Promise<FileData>{try{const result=await ideaCommand<FileData>('attachment',{id});await client.diskWrite('attachment:'+id,result);return result}catch(error){const cached=await client.diskRead('attachment:'+id) as FileData|undefined;if(cached)return cached;throw error}}
export async function downloadAttachment(client:IdeasClient,file:Attachment){const result=await attachmentData(client,file.id),url=URL.createObjectURL(attachmentBlob(result));const a=document.createElement('a');a.href=url;a.download=file.name;a.click();setTimeout(()=>URL.revokeObjectURL(url),2000)}
export async function attachmentUrl(client:IdeasClient,id:string){return URL.createObjectURL(attachmentBlob(await attachmentData(client,id)))}
export interface ConnectionState{running:boolean;address:string;qr_svg:string|null;expires:number|null;network_available:boolean;pending:{id:string;name:string;expires:number}[];devices:{id:string;name:string;created_at:number;last_seen:number;revoked_at:number|null;disconnected_at:number|null}[]}
