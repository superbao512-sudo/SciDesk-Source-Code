import {uiText} from '../../../locales/language';
import {useCallback,useEffect,useRef,useState,type CSSProperties,type PointerEvent} from 'react';
import {useLocation} from 'react-router-dom';
import {getDocument,GlobalWorkerOptions,TextLayer,type PDFDocumentProxy} from 'pdfjs-dist';
import worker from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import {projectCommand} from '../../projects/projectService';
import {SendManual,type Excerpt} from './sendManual';
import {useNativeFileDrop} from '../../files/useFileDragDrop';
import './research.css';
import {PdfTranslation} from './PdfTranslation';

GlobalWorkerOptions.workerSrc=worker;
type Rect=[number,number,number,number];
type Note={id:string;page:number;quote:string;comment:string;rect:Rect|null};
type Book={key:string;name:string;base64:string;notes:Note[]};
type OpenBook=Omit<Book,'base64'>&{pdf:PDFDocumentProxy};
const clamp=(x:number)=>Math.max(0,Math.min(1,x));
const box=(a:number[],b:number[]):Rect=>[Math.min(a[0],b[0]),Math.min(a[1],b[1]),Math.abs(a[0]-b[0]),Math.abs(a[1]-b[1])];
const position=(r:Rect):CSSProperties=>({left:100*r[0]+'%',top:100*r[1]+'%',width:100*r[2]+'%',height:100*r[3]+'%'});
const clean=(s:string)=>s.replace(/[[\]\\\r\n]/g,' ');

export default function PdfReader(){
 const location=useLocation(),[book,setBook]=useState<OpenBook|null>(null),[page,setPage]=useState(1),[scale,setScale]=useState(1.25),[ready,setReady]=useState(false);
 const [mode,setMode]=useState<'text'|'crop'>('text'),[rect,setRect]=useState<Rect|null>(null),[quote,setQuote]=useState(''),[comment,setComment]=useState('');
 const [notice,setNotice]=useState(''),[busy,setBusy]=useState(false),[send,setSend]=useState<Excerpt|null>(null);
 const area=useRef<HTMLDivElement>(null),sheet=useRef<HTMLDivElement>(null),canvas=useRef<HTMLCanvasElement>(null),layer=useRef<HTMLDivElement>(null);
 const drag=useRef<number[]|null>(null),sequence=useRef(0),lock=useRef(false);
 const open=useCallback(async(args:Record<string,unknown>,requested=1)=>{
  const seq=++sequence.current;setBusy(true);setNotice('');
  try{const f=await projectCommand<Book|null>('research_pdf',args);if(!f||seq!==sequence.current)return;
   const data=Uint8Array.from(atob(f.base64),ch=>ch.charCodeAt(0));
   const asset=(dir:string)=>new URL('/pdf-assets/'+dir+'/',document.baseURI).href;
   const task=getDocument({data,cMapUrl:asset('cmaps'),cMapPacked:true,standardFontDataUrl:asset('standard_fonts'),wasmUrl:asset('wasm'),iccUrl:asset('iccs'),useWasm:false,enableXfa:false,maxImageSize:24000000});
   task.onPassword=()=>{if(seq===sequence.current)setNotice(uiText("此 PDF 受密码保护，请打开已解锁的本地副本"));void task.destroy();if(seq===sequence.current)setBusy(false)};
   const pdf=await task.promise;if(seq!==sequence.current){void pdf.loadingTask.destroy();return}
   setReady(false);setRect(null);setQuote('');setComment('');setPage(Math.min(pdf.numPages,Math.max(1,Math.floor(requested)||1)));
   setBook({pdf,key:f.key,name:f.name,notes:f.notes});
  }catch(e){if(seq===sequence.current)setNotice(String(e))}
  finally{if(seq===sequence.current)setBusy(false)}
 },[]);
 useEffect(()=>{const params=new URLSearchParams(location.search);const key=params.get('key'),source=location.state?.source;
  let active=true;const token=sequence;
  if(key||source)queueMicrotask(()=>{if(active)void open(key?{key}:source,Number(params.get('page')))});
  return()=>{active=false;token.current++};
 },[location.key,location.search,location.state,open]);
 const pdf=book?.pdf;
 useEffect(()=>()=>{if(pdf)void pdf.loadingTask.destroy()},[pdf]);
 useEffect(()=>{if(!pdf||!canvas.current||!layer.current||!sheet.current)return;
  let active=true;let render:ReturnType<Awaited<ReturnType<typeof pdf.getPage>>['render']>|undefined,text:TextLayer|undefined;
  const el=canvas.current,words=layer.current,paper=sheet.current;
  setReady(false);words.replaceChildren();
  void(async()=>{const p=await pdf.getPage(page);if(!active)return;
   const initial=p.getViewport({scale:1}),effective=Math.min(scale,2400/Math.max(initial.width,initial.height));
   const v=p.getViewport({scale:effective}),dpi=Math.min(window.devicePixelRatio||1,2,3000/Math.max(v.width,v.height));
   paper.style.width=v.width+'px';paper.style.height=v.height+'px';paper.style.setProperty('--total-scale-factor',String(effective));
   el.width=Math.max(1,Math.round(v.width*dpi));el.height=Math.max(1,Math.round(v.height*dpi));
   render=p.render({canvas:el,viewport:v,transform:[dpi,0,0,dpi,0,0]});await render.promise;if(!active)return;
   setReady(true);const content=await p.getTextContent();if(!active)return;
   text=new TextLayer({textContentSource:content,container:words,viewport:v});await text.render();
  })().catch(e=>{if(active)setNotice('页面读取失败：'+String(e))});
  return()=>{active=false;render?.cancel();text?.cancel()};
 },[pdf,page,scale]);
 const drop=useCallback((paths:string[])=>{if(paths.length!==1){setNotice(uiText("每次打开一份 PDF"));return}void open({path:paths[0]})},[open]);
 useNativeFileDrop(area,!busy,drop,setNotice);
 const go=(n:number)=>{if(!book)return;setPage(Math.min(book.pdf.numPages,Math.max(n,1)));setRect(null);setQuote('');setComment('');setNotice('');drag.current=null};
 const point=(ev:PointerEvent<HTMLDivElement>)=>{const b=ev.currentTarget.getBoundingClientRect();return [clamp((ev.clientX-b.left)/b.width),clamp((ev.clientY-b.top)/b.height)]};
 const pickText=()=>{const s=window.getSelection();if(!s?.rangeCount||!layer.current?.contains(s.anchorNode)||!layer.current.contains(s.focusNode))return;
  setQuote(s.toString().trim().slice(0,20000));setRect(null)};
 const action=async(fn:()=>Promise<void>)=>{if(lock.current)return;lock.current=true;setBusy(true);setNotice('');try{await fn()}catch(e){setNotice(String(e))}finally{lock.current=false;setBusy(false)}};
 function excerpt():Excerpt{
  if(!book)throw Error('请先打开 PDF');
  const reference='['+clean(book.name)+' · 第 '+page+' 页](#pdf/'+book.key+'/'+page+')';
  let image:string|undefined;
  if(rect&&canvas.current){const el=canvas.current,cut=document.createElement('canvas');cut.width=Math.max(1,Math.round(rect[2]*el.width));cut.height=Math.max(1,Math.round(rect[3]*el.height));cut.getContext('2d')!.drawImage(el,rect[0]*el.width,rect[1]*el.height,rect[2]*el.width,rect[3]*el.height,0,0,cut.width,cut.height);image=cut.toDataURL('image/png')}
  return {image,alt:book.name+' 第 '+page+' 页插图',markdown:[quote?quote.split('\n').map(s=>'> '+s).join('\n'):'',comment,reference].filter(Boolean).join('\n\n')};
 }
 const idea=async()=>{if(!book)return;const item=excerpt();const {getIdeasClient,uid}=await import('../../ideas/ideasService');const client=await getIdeasClient(),id=uid();
  const v=client.state.canvasDrafts.hall??client.state.canvases.hall??{x:0,y:0,zoom:1};const shift=(client.view().length%8)*20;
  await client.enqueue(id,{type:item.image?'image':'text',title:book.name+' · 第 '+page+' 页',content:item.markdown,canvas_x:(160-v.x)/v.zoom+shift,canvas_y:(100-v.y)/v.zoom+shift,width:320,height:240});
  if(item.image){const bytes=Uint8Array.from(atob(item.image.split(',')[1]),c=>c.charCodeAt(0));try{await client.upload(id,new File([bytes],'摘图.png',{type:'image/png'}))}catch(e){throw Error('摘录文字已加入想法，图片添加失败：'+String(e))}}
  await client.persist();setNotice(uiText("摘录已加入「想法」"));
 };
 const has=ready&&(!!rect||!!quote.trim()||!!comment.trim());
 return <div className="rs-tool rs-reader" ref={area}><header><h1>{uiText("PDF 阅读与摘录")}</h1><button className="button primary" disabled={busy} onClick={()=>void open({})}>{uiText("打开 PDF")}</button>{book&&<b>{book.name}</b>}</header>
 {notice&&<p className="rs-notice" role="status">{notice}</p>}
 {!book?<p className="rs-empty">{busy?uiText("正在读取 PDF…"):uiText("打开本地论文，或将 PDF 拖到这里。")}</p>:<>
 <div className="rs-buttons"><button className="button" disabled={busy||page<=1} onClick={()=>go(page-1)}>{uiText("上一页")}</button><label>{uiText("第")}<input aria-label={uiText("页码")} type="number" min={1} max={book.pdf.numPages} value={page} onChange={e=>go(Math.floor(Number(e.target.value))||1)}/> / {book.pdf.numPages}{uiText("页")}</label><button className="button" disabled={busy||page>=book.pdf.numPages} onClick={()=>go(page+1)}>{uiText("下一页")}</button>
 <select aria-label={uiText("缩放")} value={scale} onChange={e=>setScale(+e.target.value)}>{[.75,1,1.25,1.5,2].map(s=><option key={s} value={s}>{s*100}%</option>)}</select>
 {(['text','crop'] as const).map(m=><button className="button" aria-pressed={mode===m} key={m} onClick={()=>{setMode(m);setQuote('');setRect(null)}}>{m==='text'?uiText("选取文字"):uiText("框选插图")}</button>)}</div>
 <div className="rs-pdf-layout"><div className="rs-viewport"><div className="rs-sheet" ref={sheet}><canvas ref={canvas}/><div className="rs-text-layer" ref={layer} onPointerUp={pickText} onKeyUp={pickText}/>
 {book.notes.filter(n=>n.page===page&&n.rect).map(n=><div key={n.id} className="rs-mark" style={position(n.rect!)} title={n.comment}/>)}
 {rect&&<div className="rs-box" style={position(rect)}/>}
 {mode==='crop'&&ready&&<div className="rs-crop" onPointerDown={e=>{if(e.button!==0)return;drag.current=point(e);setRect(null);setQuote('');e.currentTarget.setPointerCapture(e.pointerId)}} onPointerMove={e=>{if(drag.current)setRect(box(drag.current,point(e)))}} onPointerCancel={()=>{drag.current=null;setRect(null)}} onPointerUp={e=>{if(!drag.current)return;const r=box(drag.current,point(e));drag.current=null;setRect(r[2]>.002&&r[3]>.002?r:null)}}/>}</div></div>
 <aside className="rs-notes"><h2>{uiText("摘录与批注")}</h2>{quote&&<blockquote>{quote}</blockquote>}{rect&&<p>{uiText("已框选本页插图")}</p>}
 <PdfTranslation key={book.key} page={page} quote={quote} onAdopt={text=>setComment(old=>[old,'译文：'+text].filter(Boolean).join('\n\n'))}/>
 <label>{uiText("批注")}<textarea value={comment} maxLength={12000} onChange={e=>setComment(e.target.value)} placeholder={uiText("记录这页的结论或疑问")}/></label>
 <div className="rs-buttons"><button className="button" disabled={!has||busy} onClick={()=>void action(async()=>{const n=await projectCommand<Note>('research_note',{key:book.key,note:{page,quote,comment,rect}});
  setBook(prev=>prev&&prev.key===book.key?{...prev,notes:[...prev.notes,n]}:prev);setNotice(uiText("批注已保存在本机"))})}>{uiText("保存批注")}</button>
 <button className="button" disabled={!has||busy} onClick={()=>void action(idea)}>{uiText("发到想法")}</button><button className="button" disabled={!has||busy} onClick={()=>setSend(excerpt())}>{uiText("插入手册")}</button></div>
 <h2>{uiText("已存批注 ·")}{book.notes.length}</h2>{book.notes.map(n=><button key={n.id} className="rs-note" onClick={()=>{go(n.page);setRect(n.rect);setQuote(n.quote);setComment(n.comment);setMode(n.rect?'crop':'text')}}><b>{uiText("第")}{n.page}{uiText("页")}</b> {n.comment||n.quote||uiText("框选插图")}</button>)}</aside></div></>}
 {send&&<SendManual item={send} onClose={()=>setSend(null)}/>}</div>
}
