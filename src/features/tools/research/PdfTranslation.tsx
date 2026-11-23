import {uiText} from '../../../locales/language';
import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';

import {glossary,physicsPhrase,searchTerms,termsIn} from './condensedMatter';

type ModelFile=[string,boolean];
type Translation={source:string;text:string;terms:string[]};

export function PdfTranslation({quote,page,onAdopt}:{quote:string;page:number;onAdopt:(s:string)=>void}){
 const worker=useRef<Worker|null>(null);
 const alive=useRef(false);
 const [files,setFiles]=useState<ModelFile[]>([]);
 const [busy,setBusy]=useState('');
 const [error,setError]=useState('');
 const [translation,setTranslation]=useState<Translation|null>(null);
 const [repair,setRepair]=useState(false);
 const [external,setExternal]=useState(false);
 const [query,setQuery]=useState('');
 const [endpoint,setEndpoint]=useState(()=>localStorage.getItem('pdf.model.endpoint')||'');
 const [model,setModel]=useState(()=>localStorage.getItem('pdf.model.name')||'');
 const [token,setToken]=useState('');
 const matches=termsIn(quote);
 const direct=physicsPhrase(quote);
 const source=JSON.stringify([page,quote,external,external?endpoint:'',external?model:'']);
 useEffect(()=>{
  alive.current=true;
  invoke<ModelFile[]>('translation_files').then(list=>{if(alive.current)setFiles(list)}).catch(e=>{if(alive.current)setError(String(e))});
  return()=>{alive.current=false;worker.current?.terminate();worker.current=null};
 },[]);
 const ready=files.length>0&&files.every(f=>f[1])&&!repair;
 const install=async()=>{
  setError('');
  try{
   for(const [i,[name]] of files.entries()){
    if(!alive.current)return;
    setBusy('准备模型文件 '+(i+1)+' / '+files.length);
    await invoke('translation_install',{name});
   }
   if(alive.current){setFiles(await invoke<ModelFile[]>('translation_files'));setRepair(false);}
  }catch(e){if(alive.current)setError(String(e))}
  finally{if(alive.current)setBusy('')}
 };
 const translate=async()=>{
  setError('');setTranslation(null);setBusy(external?'正在请求所选模型…':'正在本机翻译…');
  try{
   if(external||direct){
    const terms=matches.map(e=>e.en+' · '+e.zh);
    let text=direct||'';
    if(external){
     localStorage.setItem('pdf.model.endpoint',endpoint.trim());localStorage.setItem('pdf.model.name',model.trim());
     text=await invoke<string>('translate_with_provider',{endpoint,model,token,text:quote,terms:terms.join('\n')});
    }
    if(alive.current){setTranslation({source,text,terms});setBusy('')}return;
   }
   let buffers:[string,ArrayBuffer][]|undefined;
   if(!worker.current){
    buffers=[];
    for(const [name] of files){
     const bytes=await invoke<ArrayBuffer>('translation_read',{name});
     if(!alive.current)return;
     buffers.push([name,bytes instanceof ArrayBuffer?bytes:new Uint8Array(bytes).buffer]);
    }
    worker.current=new Worker('/pdf-translation-worker.js',{type:'module'});
   }
   const current=worker.current;
   const fail=(message:string)=>{current.terminate();worker.current=null;setError(message);setBusy('')};
   current.onerror=e=>fail(e.message||'翻译引擎启动失败');
   current.onmessage=({data})=>{
    if(data.error){fail(data.error);return}
    setTranslation({source,text:data.result,terms:data.terms});setBusy('');
   };
   const segments=Array.from(new Intl.Segmenter('en',{granularity:'sentence'}).segment(quote.replace(/([a-z])-\s*\n\s*([a-z])/g,'$1$2').replace(/\s+/g,' ').trim()),s=>physicsPhrase(s.segment));
   current.postMessage({text:quote,files:buffers,segments,glossary:matches.map(e=>[e.en,e.zh])},buffers?.map(f=>f[1])??[]);
  }catch(e){if(alive.current){setError(String(e));setBusy('');if(!external)setRepair(true)}}
 };
 const result=translation?.source===source?translation:null;
 return <section aria-label={uiText("物理英译中")}><h2>{uiText("物理英译中")}</h2>
 <details><summary>{uiText("凝聚态英汉词典（")}{glossary.length}{uiText("词条）")}</summary>
 <input aria-label={uiText("查术语")} value={query} placeholder={uiText("输入英文、中文或分类")} onChange={e=>setQuery(e.target.value)}/>
 <div style={{maxHeight:220,overflowY:'auto'}}>{(query.trim()?searchTerms(query):matches.length?matches:glossary).map(e=><p key={e.en} lang="en">{e.en}<br/><strong lang="zh">{e.zh}</strong> · {e.category}</p>)}{query.trim()&&!searchTerms(query).length&&<p>{uiText("未收录这个词条")}</p>}</div></details>
 <label>{uiText("翻译方式")}<select disabled={!!busy} value={external?'external':'local'} onChange={e=>{setExternal(e.target.value==='external');setError('')}}><option value="local">{uiText("内置物理翻译（离线）")}</option><option value="external">{uiText("自选模型接口")}</option></select></label>
 {external&&<fieldset disabled={!!busy}><label>{uiText("聊天接口地址")}<input placeholder="https://…/v1/chat/completions" value={endpoint} onChange={e=>{setEndpoint(e.target.value);setToken('')}}/></label><label>{uiText("模型名")}<input value={model} onChange={e=>setModel(e.target.value)}/></label><label>{uiText("API 密钥（仅本次打开）")}<input type="password" autoComplete="off" value={token} onChange={e=>setToken(e.target.value)}/></label><small>{uiText("点击翻译会将选文和匹配的术语发送到以上地址。")}</small></fieldset>}
 {ready||external||direct?<button className="button" disabled={!quote.trim()||quote.length>4000||!!busy||(external&&(!model.trim()||!endpoint.trim()))} onClick={()=>void translate()}>{uiText("译为中文")}</button>
 :<button className="button" disabled={!files.length||!!busy} onClick={()=>void install()}>{repair?uiText("修复模型包"):uiText("下载本地英译中模型（约 120 MB）")}</button>}
 {quote.length>4000&&<p>{uiText("请将选文分为 4000 字符以内的段落。")}</p>}
 {busy&&<p role="status">{busy}</p>}{error&&<p role="alert">{error}</p>}
 {result&&<><p style={{whiteSpace:'pre-wrap'}}>{result.text}</p>{result.terms.length>0&&<small>{result.terms.join('；')}</small>}
 <div><button className="button" onClick={()=>onAdopt(result.text)}>{uiText("将译文加入批注")}</button></div></>}
 </section>;
}
