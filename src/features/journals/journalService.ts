import {invoke,isTauri} from '@tauri-apps/api/core';
import type {PaperJournal,PaperJournalInput,PaperFeedListOptions,PaperFeedPage,PaperTopic,PaperTopicInput,Catalog,SyncRun,LibraryJournal,ScholarProfile,PaperSourceStatus} from './types';
export async function journalCommand<T>(op:string,args:object={}):Promise<T>{
 if(isTauri())return invoke<T>('journal_command',{op,args});
 const response=await fetch('/__scidesk/journals',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({op,args})});
 let result;try{result=await response.json()}catch{throw new Error('期刊服务响应无效，请检查开发服务')}
 if(!response.ok||!result.ok)throw new Error(result.error||'期刊请求失败');return result.value as T;
}
export const journalService={
 sources:()=>journalCommand<PaperSourceStatus[]>('sources'),
 profile:()=>journalCommand<ScholarProfile>('profile'),
 bindProfile:(url:string)=>journalCommand<ScholarProfile>('profile_bind',{url}),
 updateProfile:()=>journalCommand<ScholarProfile>('profile_update'),
 library:()=>journalCommand<LibraryJournal[]>('library'),
 avatar:()=>journalCommand<string|null>('avatar'),
 saveAvatar:(data:string)=>journalCommand<string>('save_avatar',{data}),
 list:()=>journalCommand<PaperJournal[]>('list'),
 save:(id:string|null,input:PaperJournalInput)=>journalCommand<PaperJournal>('save',{id,input}),
 remove:(id:string)=>journalCommand<PaperJournal>('remove',{id}),
 topics:()=>journalCommand<PaperTopic[]>('topics'),
 saveTopic:(id:string|null,input:PaperTopicInput)=>journalCommand<PaperTopic>('save_topic',{id,input}),
 removeTopic:(id:string)=>journalCommand<void>('remove_topic',{id}),
 linkTopics:(id:string,topicIds:string[])=>journalCommand<PaperJournal>('journal_topics',{id,topicIds}),
 feed:(options:PaperFeedListOptions)=>journalCommand<PaperFeedPage>('feed',options),
 catalog:()=>journalCommand<Catalog>('catalog'),
 sync:(source:string,id?:string)=>journalCommand<SyncRun>('sync',{source,id}),
 status:(id:string,status:string)=>journalCommand<void>('status',{id,status}),
};
export function errorMessage(error:unknown){return error instanceof Error?error.message:String(error)}
export const statusLabel=(status:string)=>({running:'正在同步',success:'同步成功',partial_success:'部分成功',failed:'同步失败',network_error:'网络错误',http_error:'请求失败',parse_error:'解析失败',rate_limited:'请求受限',timeout:'请求超时',not_configured:'未配置',cancelled:'已中断',never:'尚未同步'}[status]||status);
export function dateLabel(value?:string|null){if(!value)return '尚无记录';const date=new Date(value);return Number.isNaN(date.getTime())?'日期未知':date.toLocaleString('zh-CN',{hour12:false})}
export function articleLink(url:string){try{const parsed=new URL(url);return parsed.protocol==='https:'&&!parsed.username&&!parsed.password&&!parsed.port&&['doi.org','dx.doi.org','arxiv.org','export.arxiv.org'].includes(parsed.hostname)?url:null}catch{return null}}
export async function openArticle(url:string){if(!articleLink(url))throw new Error('原文链接无效');if(isTauri())await journalCommand('open',{url});else window.open(url,'_blank','noopener,noreferrer')}

export async function openScholar(){await journalCommand('open_profile')}

export function journalWebsiteLink(url:string|null|undefined){try{if(!url)return null;const parsed=new URL(url);return parsed.protocol==='https:'&&!!parsed.hostname&&!parsed.username&&!parsed.password?parsed.href:null}catch{return null}}
export async function openJournalWebsite(journal:Pick<PaperJournal,'id'|'websiteUrl'>){const url=journalWebsiteLink(journal.websiteUrl);if(!url)throw new Error('期刊官网链接无效，请在编辑期刊中检查 HTTPS 地址');await journalCommand('open_journal',{id:journal.id})}
