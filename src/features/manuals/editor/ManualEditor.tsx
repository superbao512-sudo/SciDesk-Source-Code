import {uiText} from '../../../locales/language';
import {useEffect,useRef,useState,type MouseEvent} from 'react';
import {useLocation,useNavigate} from 'react-router-dom';
import {ArrowLeft,BookOpen,FilePlus2,FolderOpen,ImagePlus,List,Pencil,Save,Sigma} from 'lucide-react';
import {isTauri} from '@tauri-apps/api/core';
import {SortableListHandle} from '../../../components/common/SortableListHandle';
import {NameDialog} from '../../projects/ProjectDialogs';
import {manualCommand} from '../manualService';
import {bookCommand,compileBook,renderChapter,standalone,type Scope,type Book,type Source,type Page} from './bookRender';
import './paper.css';
import './editor.css';

type Place={scope?:Scope;reading?:boolean;chapter?:string;back?:string;fresh?:boolean};
type DocumentState=Source&{chapter:string;saved:string};
type Compilation=Awaited<ReturnType<typeof compileBook>>;
const draftKey=(scope:Scope,chapter:string)=>JSON.stringify(['scidesk-manual-draft',scope.id,scope.linkId,scope.base||'',chapter]);
const emptyChapterMessage='当前章节为空，请先写入正文，再生成整本预览。';
const message=(e:unknown)=>e instanceof Error?e.message:String(e);

export default function ManualEditor(){
 const location=useLocation(),navigate=useNavigate(),place=(location.state||{}) as Place;
 if(!place.scope)return <NewBook/>;
 return <Editor key={location.key} scope={place.scope} place={place} back={()=>navigate(place.back||'/tools')}/>;
}

function NewBook(){
 const navigate=useNavigate();
 const [error,setError]=useState(''),[attempt,setAttempt]=useState(0);
 const request=useRef<Promise<{scope:Scope;chapter:string}>|null>(null);
 useEffect(()=>{
  let active=true;
  // A repeated mount effect must subscribe to the same file creation.
  request.current??=manualCommand<{scope:Scope;chapter:string}>('book_new');
  void request.current.then(result=>{
   if(active)navigate('/tools/manual-editor',{replace:true,state:{...result,fresh:true,back:'/tools'}});
  }).catch(reason=>{if(active)setError(message(reason))});
  return()=>{active=false};
 },[navigate,attempt]);
 return <section className="te-launch">{error?<><p role="alert">{error}</p><button className="button" onClick={()=>{request.current=null;setError('');setAttempt(n=>n+1)}}>{uiText("重新创建")}</button></>:<p role="status">{uiText("正在新建 Markdown 文件…")}</p>}</section>;
}

function Editor({scope,place,back}:{scope:Scope;place:Place;back:()=>void}){
 const navigate=useNavigate();
 const [progress,setProgress]=useState(''); 
 const resultHeading=useRef<HTMLHeadingElement>(null);
 const [book,setBook]=useState<Book|null>(null),[doc,setDoc]=useState<DocumentState|null>(null);
 const [reading,setReading]=useState(!!place.reading),[toc,setToc]=useState(false);
 const [busy,setBusy]=useState(true),[error,setError]=useState(''),[notice,setNotice]=useState('');
 const [dialog,setDialog]=useState<'create'|'rename'|null>(null),[hover,setHover]=useState('');
 const [page,setPage]=useState<Page|null>(null),[compiled,setCompiled]=useState<Compilation|null>(null);
 const textArea=useRef<HTMLTextAreaElement>(null),imageInput=useRef<HTMLInputElement>(null);
 const preview=useRef<HTMLDivElement>(null),jump=useRef('');
 const initial=useRef({scope,place});
 const loadChapter=async(s:Scope,chapter:string)=>{
  const disk=await bookCommand<Source>(s,'read',{chapter});
  let restored:Partial<Source>|null=null;
  try{restored=JSON.parse(sessionStorage.getItem(draftKey(s,chapter))||'null')}catch{/* Draft storage is optional. */}
  const draft=restored&&typeof restored.text==='string'&&typeof restored.hash==='string'?restored as Source:null;
  return {chapter,text:draft?.text??disk.text,hash:draft?.hash??disk.hash,saved:disk.text};
 };
 useEffect(()=>{let alive=true;const {scope,place}=initial.current;
  void(async()=>{const b=await bookCommand<Book>(scope,'list');const ch=place.chapter||b.chapters[0];
   if(alive)setBook(b);
   if(ch){const d=await loadChapter(scope,ch);if(alive){setDoc(d);if(d.saved!==d.text)setNotice(uiText("已恢复本窗口的未保存草稿"))}}
  })().catch(e=>{if(alive)setError(message(e))}).finally(()=>{if(alive)setBusy(false)});
  return()=>{alive=false};
 },[]);
 useEffect(()=>{if(!book||!doc)return;let alive=true;const timer=setTimeout(()=>{
  void renderChapter(scope,book.chapters,doc.chapter,doc.text).then(result=>{if(alive)setPage(result)}).catch(e=>{if(alive)setError(message(e))});
 },110);return()=>{alive=false;clearTimeout(timer)}},[book,doc,scope]);
 useEffect(()=>{if(page&&jump.current){preview.current?.querySelector('#'+CSS.escape(jump.current))?.scrollIntoView({block:'start'});jump.current=''}},[page]);
 useEffect(()=>{if(compiled)resultHeading.current?.focus()},[compiled]);
 useEffect(()=>{if(doc?.chapter)sessionStorage.setItem('research.currentChapter',JSON.stringify({scope,chapter:doc.chapter}))},[scope,doc?.chapter]);
 const [conflict,setConflict]=useState<string|null>(null);
 const dirty=!!doc&&doc.text!==doc.saved;
 const run=async(task:()=>Promise<void>)=>{if(busy)return;setBusy(true);setError('');setNotice('');try{await task()}catch(e){setError(message(e))}finally{setBusy(false)}};
 const change=(value:string)=>{if(!doc)return;
  if(error===emptyChapterMessage&&value.trim())setError('');
  setDoc({...doc,text:value});
  try{const key=draftKey(scope,doc.chapter);if(value===doc.saved)sessionStorage.removeItem(key);else sessionStorage.setItem(key,JSON.stringify({hash:doc.hash,text:value}))}catch{setNotice(uiText("临时草稿空间已满，请按 Ctrl+S 保存到文件"))}
 };
 const save=async()=>{if(!doc||!dirty)return;
  const {hash}=await bookCommand<{hash:string}>(scope,'save',{chapter:doc.chapter,hash:doc.hash,text:doc.text});
  sessionStorage.removeItem(draftKey(scope,doc.chapter));setDoc({...doc,hash,saved:doc.text});
 };
 useEffect(()=>{const key=(e:KeyboardEvent)=>{if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'){e.preventDefault();if(!dialog)void run(save)}};
  const leave=(e:BeforeUnloadEvent)=>{if(dirty){e.preventDefault();e.returnValue=''}};
  window.addEventListener('keydown',key);window.addEventListener('beforeunload',leave);return()=>{window.removeEventListener('keydown',key);window.removeEventListener('beforeunload',leave)};
 });
 const select=async(chapter:string)=>{await save();const d=await loadChapter(scope,chapter);setDoc(d);setConflict(null);setPage(null);setCompiled(null);setToc(false)};
 const reorder=(from:string,to:string)=>{if(!book||busy||!to||from===to)return;void run(async()=>{
  const order=[...book.chapters];order.splice(order.indexOf(from),1);order.splice(book.chapters.indexOf(to),0,from);
  await bookCommand(scope,'order',{order,expected:book.chapters});setBook({...book,chapters:order});setCompiled(null);
 })};
 const insert=(value:string,start=textArea.current?.selectionStart??0,end=textArea.current?.selectionEnd??start)=>{
  if(!doc)return;change(doc.text.slice(0,start)+value+doc.text.slice(end));requestAnimationFrame(()=>{textArea.current?.focus();textArea.current?.setSelectionRange(start+value.length,start+value.length)});
 };
 const formula=(block:boolean)=>{if(!doc||!textArea.current)return;const {selectionStart:s,selectionEnd:e}=textArea.current;
  const expr=doc.text.slice(s,e)||'E = mc^2';insert(block?'\n\n$$\n'+expr+'\n$$\n\n':'$'+expr+'$',s,e)};
 const addImage=(file:File)=>{if(!doc||!textArea.current)return;const {selectionStart:s,selectionEnd:e}=textArea.current;
  void run(async()=>{if(file.size>12*1024*1024)throw Error('图片超过 12 MB');
   const data=await new Promise<string>((resolve,reject)=>{const r=new FileReader();r.onload=()=>resolve(String(r.result).split(',')[1]);r.onerror=()=>reject(r.error);r.readAsDataURL(file)});
   const path=await bookCommand<string>(scope,'add_image',{data});
   const alt=file.name.replace(/\.[^.]*$/,'').replace(/[[\]\\\r\n]/g,'')||'插图';
   insert('!['+alt+']('+'../'.repeat(doc.chapter.split('/').length-1)+path+')',s,e);
  })};
 const openBook=()=>void run(async()=>{
  await save();
  const next=await manualCommand<Scope|null>('book_pick');
  if(next)navigate('/tools/manual-editor',{state:{scope:next,back:place.back||'/tools'}});
 });
 const compile=()=>void run(async()=>{
  setToc(false);
  if(book?.chapters.length===1&&doc&&!doc.text.trim()){
   setReading(false);textArea.current?.focus();
   throw Error(emptyChapterMessage);
  }
  setProgress('正在准备整本预览…');
  try{
   await save();
   const latest=await bookCommand<Book>(scope,'list');
   if(!latest.chapters.length)throw Error('这个文件夹还没有 Markdown 文件，请先新建章节。');
   const result=await compileBook(scope,latest,(index,total,chapter)=>setProgress('正在排版 '+index+' / '+total+' 章 · '+chapter));
   setBook(latest);setCompiled(result);
   setNotice('整本预览已生成，共 '+latest.chapters.length+' 章。可在下方导出 PDF 或 HTML。');
  }catch(reason){throw Error('无法生成整本预览：'+message(reason))}
  finally{setProgress('')}
 });
 const output=(format:'html'|'pdf')=>void run(async()=>{
  if(!compiled||!book)return;
  if(format==='html'){const path=await bookCommand<string>(scope,'html',{html:standalone(book.title,compiled.html)});setNotice('HTML 已保存至 '+path);return}
  const print=document.createElement('main');print.className='te-print book-paper';print.innerHTML=compiled.html;document.body.append(print);
  try{await document.fonts.ready;await Promise.all(Array.from(print.querySelectorAll('img'),img=>img.decode()));
   await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
   if(isTauri()){const path=await bookCommand<string>(scope,'pdf');setNotice('PDF 已保存至 '+path)}else{window.print()}
  }finally{print.remove()}
 });
 const follow=(e:MouseEvent<HTMLElement>)=>{const a=(e.target as Element).closest('a');if(!a)return;e.preventDefault();
  const hash=a.getAttribute('href')||'';
  if(/^#pdf\/[a-f0-9]{64}\/\d+$/.test(hash)){const [,key,page]=hash.split('/');void run(async()=>{await save();navigate('/tools/pdf-reader?key='+key+'&page='+page)});return}
  if(!hash.startsWith('#')){if(hash)setNotice('外部链接：'+hash);return}
  const local=e.currentTarget.querySelector('#'+CSS.escape(hash.slice(1)));if(local){local.scrollIntoView({block:'start'});return}
  const chapter=a.getAttribute('data-chapter');if(chapter){jump.current=hash.slice(1);void run(()=>select(chapter))}
 };
 return <section className="te-editor" aria-label={uiText("教学手册编辑器")}>
  <header className="te-bar"><button className="icon-button" title={uiText("返回")} aria-label={uiText("返回")} disabled={busy} onClick={()=>void run(async()=>{await save();back()})}><ArrowLeft size={18}/></button>
   <div className="te-title"><small>{uiText("教学手册编辑器")}</small><strong title={book?.root}>{book?.title||uiText("打开手册…")}</strong></div>
   <div className="te-actions"><button className="button" disabled={busy} onClick={openBook} title={uiText("打开已有手册文件夹")}><FolderOpen size={16}/>{uiText("打开手册")}</button><button className="button" disabled={!book||busy} aria-expanded={toc} onClick={()=>setToc(!toc)}><List size={16}/>{uiText("章节")}</button>
   {compiled?<button className="button" disabled={busy} onClick={()=>{setCompiled(null);setNotice('')}}>{uiText("返回章节")}</button>:<button className="button" disabled={!doc||busy} onClick={()=>setReading(!reading)}>{reading?<Pencil size={16}/>:<BookOpen size={16}/>} {reading?uiText("编辑"):uiText("阅读")}</button>}
   <button className="button" disabled={!dirty||busy} onClick={()=>void run(save)} title="Ctrl+S"><Save size={16}/>{uiText("保存")}</button>
   {!compiled&&<button className="button primary" disabled={!book||busy} title={uiText("按章节顺序生成目录和整本预览，可导出 PDF 或 HTML")} onClick={compile}>{progress?uiText("正在排版…"):uiText("整本预览与导出")}</button>}</div></header>
  {error&&<div className="te-error" role="alert">{error}{dirty&&error.includes('其他位置修改')&&<button className="button" onClick={()=>void run(async()=>{
   if(!doc)return;const disk=await bookCommand<Source>(scope,'read',{chapter:doc.chapter});setDoc({...doc,saved:disk.text,hash:disk.hash});
   sessionStorage.setItem(draftKey(scope,doc.chapter),JSON.stringify({hash:disk.hash,text:doc.text}));setReading(false);
   setNotice(uiText("草稿仍在左栏。请核对磁盘内容并合并后保存。"));setConflict(disk.text);
  })}>{uiText("查看磁盘内容并合并")}</button>}</div>}
  {progress&&<div className="te-notice" role="status">{progress}</div>}
  {notice&&<div className="te-notice" role="status">{notice}</div>}
  {toc&&book&&<aside className="te-toc"><div className="te-panelbar"><b>{uiText("章节目录")}</b>{!book.single&&<button className="button" disabled={busy} onClick={()=>setDialog('create')}><FilePlus2 size={15}/>{uiText("新建章节")}</button>}</div>{book.chapters.map((ch,i)=><div key={ch} data-chapter-id={ch} className={'te-row '+(hover===ch?'hover ':'')+(doc?.chapter===ch?'active':'')}>
   {!book.single&&<SortableListHandle id={ch} name={ch} idAttribute="data-chapter-id" onTarget={setHover} onReorder={reorder} onMove={d=>reorder(ch,book.chapters[i+d])}/>}<button disabled={busy} title={ch} onClick={()=>void run(()=>select(ch))}>{ch}</button></div>)}
   {doc&&!book.single&&<button className="button" disabled={busy} onClick={()=>setDialog('rename')}>{uiText("重命名当前章节")}</button>}</aside>}
  {compiled?<><div className="te-panelbar"><h2 className="te-result-title" tabIndex={-1} ref={resultHeading}>{uiText("整本预览 ·")}{book?.chapters.length}{uiText("章")}</h2><div><button className="button" disabled={busy} onClick={()=>output('html')}>{uiText("导出 HTML")}</button> <button className="button" disabled={busy} onClick={()=>output('pdf')}>{uiText("导出 PDF")}</button></div></div>
   {compiled.issues.length>0&&<details className="te-errors" open><summary>{compiled.issues.length}{uiText("项排版提示")}</summary>{compiled.issues.map((issue,i)=><button key={i} disabled={busy} onClick={()=>void run(async()=>{await select(issue.chapter);setReading(false)})}>{issue.chapter} · {issue.message}</button>)}</details>}
   <div className="te-scroll"><article className="book-paper" onClick={follow} dangerouslySetInnerHTML={{__html:compiled.html}}/></div></>:
   doc?<><div className="te-chapter"><span title={book?.root}>{doc.chapter}</span>{place.fresh&&<small className="te-folder" title={book?.root}>{book?.root}</small>}<small>{busy?uiText("处理中…"):dirty?uiText("有未保存修改"):uiText("已保存")}</small></div>
   <div className={'te-panes'+(reading?' reading':'')}>
    {!reading&&<div className="te-pane"><div className="te-panelbar"><b>Markdown</b><div><button title={uiText("插入行内公式")} disabled={busy} onClick={()=>formula(false)}><Sigma size={15}/>{uiText("行内")}</button><button title={uiText("插入独立公式")} disabled={busy} onClick={()=>formula(true)}>{uiText("公式块")}</button><button title={uiText("插入本地图片")} disabled={busy} onClick={()=>imageInput.current?.click()}><ImagePlus size={15}/>{uiText("插图")}</button></div></div>
     <input hidden type="file" ref={imageInput} accept="image/png,image/jpeg,image/gif,image/webp" onChange={e=>{const f=e.target.files?.[0];e.target.value='';if(f)addImage(f)}}/>
     <textarea onSelect={e=>sessionStorage.setItem('research.currentChapter',JSON.stringify({scope,chapter:doc.chapter,offset:e.currentTarget.selectionEnd}))} autoFocus={!!place.fresh} placeholder={uiText("从这里开始写 Markdown…")} aria-label={uiText("Markdown 源文")} spellCheck={false} ref={textArea} readOnly={busy} value={doc.text} onChange={e=>change(e.target.value)} onPaste={e=>{const f=Array.from(e.clipboardData.files).find(f=>f.type.startsWith('image/'));if(f&&!busy){e.preventDefault();addImage(f)}}}/></div>}
    <div className="te-pane"><div className="te-panelbar"><b>{reading?uiText("阅读"):uiText("实时排版")}</b></div><div className="te-scroll" ref={preview}>{page?<article className="book-paper" onClick={follow} dangerouslySetInnerHTML={{__html:page.html}}/>:<p>{uiText("正在排版…")}</p>}</div></div>
   </div>{conflict!==null&&<details open className="te-errors"><summary>{uiText("磁盘版本（供合并）")}</summary><pre>{conflict}</pre><button onClick={()=>setConflict(null)}>{uiText("收起对照")}</button></details>}</>:
   <div className="te-empty">{busy?uiText("正在打开章节…"):book&&!book.chapters.length?<><BookOpen size={30}/><p>{uiText("这个文件夹还没有 Markdown 章节。")}</p><button className="button primary" onClick={()=>setDialog('create')}>{uiText("新建第一章")}</button></>:uiText("请选择可以读取的章节")}</div>}
  {dialog&&book&&<NameDialog title={dialog==='create'?uiText("新建章节"):uiText("重命名章节")} label={uiText("章节文件名")} initial={dialog==='rename'?doc?.chapter.split('/').at(-1):''} detail={book.root} onClose={()=>setDialog(null)} onConfirm={async input=>{
   await save();const name=/\.(md|markdown)$/i.test(input)?input:input+'.md';
   const chapter=await bookCommand<string>(scope,dialog,{chapter:doc?.chapter,name});
   const b=await bookCommand<Book>(scope,'list');const next=await loadChapter(scope,chapter);setBook(b);setDoc(next);setConflict(null);setPage(null);setCompiled(null);setReading(false);setToc(false);
  }}/>}
 </section>;
}
