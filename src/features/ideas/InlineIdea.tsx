import {uiText} from '../../locales/language';
import {useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {ExternalLink} from 'lucide-react';
import {type Idea,type IdeasClient,validUrl} from '../../../public/ideas-mobile/core.js';
import {ideaCommand,message,downloadAttachment} from './ideasService';
import {IdeaPicture} from './IdeasDialogs';

export function InlineIdea({record,client,onChange}:{record:Idea;client:IdeasClient;onChange:(patch:Record<string,unknown>,before:Record<string,unknown>,base:number)=>Promise<unknown>}){
 const [focus,setFocus]=useState<{key:keyof Idea;version:number}|null>(null),[draft,setDraft]=useState<Partial<Idea>>({}),[tagText,setTagText]=useState(''),[error,setError]=useState('');
 const navigate=useNavigate();
 const pdf=record.content?.match(/#pdf\/([a-f0-9]{64})\/(\d+)/);
 const files=client.state.attachments.filter(a=>a.idea_id===record.id&&!a.deleted_at);
 const value=(key:keyof Idea)=>focus?.key===key?draft[key]:client.state.inputDrafts[record.id]?.[key]??record[key];
 const activate=(key:keyof Idea)=>{setDraft(old=>({...old,[key]:client.state.inputDrafts[record.id]?.[key]??record[key]}));setFocus({key,version:record.version});if(key==='tags')setTagText(record.tags.join(', '))};
 const change=(key:keyof Idea,next:unknown)=>{const previous=value(key);setDraft(old=>({...old,[key]:next}));if(key==='url'&&next&&!validUrl(String(next))){void client.stageInput(record.id,'url',next).catch(e=>setError(message(e)));setError(uiText("链接尚未完整，已保留在本机"));return}if(key==='url')void client.stageInput(record.id,'url',undefined).catch(e=>setError(message(e)));setError('');void onChange({[key]:next},{[key]:previous},client.editVersion(record.id,key,focus?.version??record.version)).catch(e=>setError(message(e)))};
 const blur=()=>{setFocus(null);void client.flush().catch(e=>setError(message(e)))};
 const upload=(list:FileList|null)=>{for(const file of Array.from(list??[]))void client.upload(record.id,file).catch(e=>setError(message(e)))};
 return <div className="idea-inline-body" onClick={e=>e.stopPropagation()}>
  <input className="idea-inline-title" aria-label={uiText("想法标题")} placeholder={uiText("标题（可选）")} maxLength={500} value={String(value('title')??'')} readOnly={!!record.deleted_at} onFocus={()=>activate('title')} onChange={e=>change('title',e.target.value)} onBlur={blur}/>
  <textarea id={'idea-content-'+record.id} className="idea-inline-content" aria-label={uiText("想法正文")} placeholder={uiText("直接输入想法…")} maxLength={200000} value={String(value('content')??'')} readOnly={!!record.deleted_at} onFocus={()=>activate('content')} onChange={e=>change('content',e.target.value)} onBlur={blur}/>
  {pdf&&<button className="idea-link" onClick={()=>navigate('/tools/pdf-reader?key='+pdf[1]+'&page='+pdf[2])}>{uiText("返回原文 · 第")}{pdf[2]}{uiText("页")}</button>}
  {record.url&&<button className="idea-link" onClick={()=>void ideaCommand('open_url',{url:record.url}).catch(e=>setError(message(e)))}><ExternalLink size={14}/>{new URL(record.url).hostname}</button>}
  {files.map(file=><div key={file.id}>{file.media_type.startsWith('image/')?<IdeaPicture client={client} id={file.id} name={file.name}/>:<button className="idea-file" onClick={()=>void downloadAttachment(client,file).catch(e=>setError(message(e)))}>{file.name}<small>{Math.ceil(file.size/1024)}{uiText("KB · 附件副本")}</small></button>}</div>)}
  <details className="idea-inline-details"><summary>{uiText("链接、归属与附件")}</summary><div>
   <label>{uiText("网页 / 论文链接")}<input aria-label={uiText("想法链接")} type="url" placeholder="https://" value={String(value('url')??'')} onFocus={()=>activate('url')} onChange={e=>change('url',e.target.value)} onBlur={blur}/></label>
   <label>{uiText("标签")}<input aria-label={uiText("想法标签")} placeholder={uiText("逗号分隔")} value={focus?.key==='tags'?tagText:record.tags.join(', ')} onFocus={()=>activate('tags')} onChange={e=>{setTagText(e.target.value);change('tags',e.target.value.split(/[,，]/).map(t=>t.trim()).filter(Boolean))}} onBlur={blur}/></label>
   <label>{uiText("所属位置")}<select aria-label={uiText("想法所属包间")} value={String(value('room_id')??'')} onFocus={()=>activate('room_id')} onChange={e=>change('room_id',e.target.value||null)} onBlur={blur}><option value="">{uiText("大厅 · 未分类")}</option>{client.view('room').filter(r=>!r.deleted_at).sort((a,b)=>a.sort_order-b.sort_order).map(r=><option key={r.id} value={r.id}>{r.name}</option>)}</select></label>
   <label>{uiText("关联项目")}<select aria-label={uiText("想法关联项目")} value={String(value('project_id')??'')} onFocus={()=>activate('project_id')} onChange={e=>change('project_id',e.target.value||null)} onBlur={blur}><option value="">{uiText("不关联项目")}</option>{client.state.projects.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}{record.project_id&&!client.state.projects.some(p=>p.id===record.project_id)&&<option value={record.project_id}>{uiText("关联项目已移除（想法保留）")}</option>}</select></label>
   <label>{uiText("添加图片或附件副本")}<input aria-label={uiText("想法附件")} type="file" multiple onChange={e=>{upload(e.target.files);e.target.value=''}}/></label>
   {files.map(file=><div className="idea-editor-attachment" key={file.id}><span>{file.name}</span><button onClick={()=>{if(window.confirm(uiText("仅移除附件副本引用，不影响原文件。继续？")))void ideaCommand('remove_attachment',{id:file.id}).then(()=>client.sync()).catch(e=>setError(message(e)))}}>{uiText("移除副本")}</button></div>)}
  </div></details>
  {client.state.uploads.filter(u=>u.idea_id===record.id).map(u=><small key={u.id}>{u.name}：{u.error||uiText("本机已保存，等待上传")}</small>)}
  {error&&<small role="alert" className="form-error">{error}</small>}
 </div>;
}
