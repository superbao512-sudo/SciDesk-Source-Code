import {uiText} from '../../../locales/language';
import {useEffect,useRef,useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {manualCommand} from '../../manuals/manualService';
import {bookCommand,type Scope,type Book,type Source} from '../../manuals/editor/bookRender';
import {Modal} from '../../../components/common/Modal';
export type Excerpt={markdown:string;image?:string;alt?:string};
type Target={scope:Scope;chapter:string;offset?:number};
export function SendManual({item,onClose}:{item:Excerpt;onClose:()=>void}){
 const navigate=useNavigate(),[book,setBook]=useState<Book|null>(null),[target,setTarget]=useState<Target|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(true);
 const mounted=useRef(true);
 const load=async(t:Target)=>{const b=await bookCommand<Book>(t.scope,'list');if(!mounted.current)return;
  setBook(b);setTarget({...t,chapter:b.chapters.includes(t.chapter)?t.chapter:b.chapters[0]||''})};
 useEffect(()=>{mounted.current=true;let active=true;
  void(async()=>{const last=sessionStorage.getItem('research.currentChapter');if(last&&active)await load(JSON.parse(last))})().catch(e=>{if(active)setError(String(e))}).finally(()=>{if(active)setBusy(false)});
  return()=>{active=false;mounted.current=false};
 },[]);
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError('');try{await fn()}catch(e){setError(String(e))}finally{if(mounted.current)setBusy(false)}};
 const send=async()=>{if(!target||!target.chapter)return;const {scope,chapter}=target;
  const disk=await bookCommand<Source>(scope,'read',{chapter});
  const key=JSON.stringify(['scidesk-manual-draft',scope.id,scope.linkId,scope.base||'',chapter]);
  const raw=sessionStorage.getItem(key),draft=raw?JSON.parse(raw) as Source:null;
  if(draft&&draft.hash!==disk.hash)throw Error('此章节有外部修改，请先在手册编辑器合并草稿');
  const source=draft?.text??disk.text;let md=item.markdown;
  if(item.image){const path=await bookCommand<string>(scope,'add_image',{data:item.image.split(',')[1]});
   md='!['+(item.alt||'插图').replace(/[[\]\\\r\n]/g,' ')+']('+'../'.repeat(chapter.split('/').length-1)+path+')\n\n'+md}
  const at=Math.min(source.length,Math.max(0,target.offset??source.length));
  const text=source.slice(0,at)+'\n\n'+md.trim()+'\n\n'+source.slice(at);
  await bookCommand(scope,'save',{chapter,hash:disk.hash,text});sessionStorage.removeItem(key);
  navigate('/tools/manual-editor',{state:{scope,chapter,back:'/manuals/'+scope.id}});
 };
 return <Modal title={uiText("插入教学手册")} onClose={onClose} closeDisabled={busy} footer={<div className="modal-actions"><button className="button" disabled={busy} onClick={onClose}>{uiText("取消")}</button><button className="button primary" disabled={busy||!target?.chapter} onClick={()=>void act(send)}>{uiText("插入并打开章节")}</button></div>}>
 <div className="form">{book&&target&&<><b>{book.title}</b><small>{book.root}</small><label>{uiText("目标章节")}<select value={target.chapter} onChange={e=>setTarget({...target,chapter:e.target.value,offset:undefined})}>{book.chapters.map(ch=><option key={ch}>{ch}</option>)}</select></label></>}
 <button className="button" disabled={busy} onClick={()=>void act(async()=>{const s=await manualCommand<Scope|null>('book_pick');if(s)await load({scope:s,chapter:''})})}>{uiText("选择手册文件夹")}</button>
 <button className="button" disabled={busy} onClick={()=>void act(async()=>{await load(await manualCommand<Target>('book_new'))})}>{uiText("新建手册")}</button>
 {book&&!book.chapters.length&&target&&<button className="button" disabled={busy} onClick={()=>void act(async()=>{await bookCommand(target.scope,'create',{name:'第一章.md'});await load({...target,chapter:'第一章.md'})})}>{uiText("创建第一章")}</button>}
 {error&&<p role="alert">{error}</p>}</div></Modal>
}
