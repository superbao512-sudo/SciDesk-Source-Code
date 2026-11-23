import {useState} from 'react';
import {learningCommand} from './learningService';

type Bookmark={title:string;url:string;category1:string;category2:string};
function readBookmarks(html:string,unsorted:string){
 const template=document.createElement('template');
 template.innerHTML=html;
 const paths=new Map<Element,string[]>(),nextFolder=new Map<Element,string>();
 const rows:Bookmark[]=[];
 let invalid=0;
 for(const node of template.content.querySelectorAll('dl,h3,a[href]')){
  const parent=node.parentElement?.closest('dl');
  const path=parent?paths.get(parent)??[]:[];
  if(node.tagName==='DL'){
   const folder=parent?nextFolder.get(parent):undefined;
   paths.set(node,folder?[...path,folder]:path);
   if(parent)nextFolder.delete(parent);
  }else if(parent&&node.tagName==='H3'){
   nextFolder.set(parent,(node.textContent??'').trim());
  }else if(parent&&node.tagName==='A'){
   const url=(node.getAttribute('href')??'').trim();
   try{const parsed=new URL(url);if(!['https:','http:'].includes(parsed.protocol)||parsed.username||parsed.password||/\s/.test(url))throw Error()}
   catch{invalid++;continue}
   const folders=path.filter((name,i)=>i!==0||!['Bookmarks bar','Bookmarks toolbar','Favorites bar','书签栏','收藏夹栏','书签工具栏'].includes(name));
   rows.push({url,title:(node.textContent??'').trim()||url,category1:folders[0]||unsorted,category2:folders.slice(1).join(' / ')});
  }
 }
 return {rows,invalid};
}
export function BookmarkImport({onImported,empty}:{onImported:()=>Promise<void>;empty:boolean}){
 const [preview,setPreview]=useState<ReturnType<typeof readBookmarks>|null>(null),[message,setMessage]=useState(''),[busy,setBusy]=useState(false);
 const prompt='请整理附件中的浏览器书签，保留每个原始网址和标题。按主题建立一级文件夹（如 VASP、Quantum ESPRESSO、声子与热输运），按用途建立二级文件夹（如官方文档、入门教程、计算实例）。不确定的放入「待整理」。请返回可下载的 UTF-8 Netscape 书签 HTML 文件，使用 DL、DT、H3 和 A HREF，方便直接导入 SciDesk。';
 async function choose(file:File){
  setBusy(true);setMessage('');setPreview(null);
  try{if(file.size>8*1024*1024)throw Error('请选择小于 8 MB 的书签文件。');
   const result=readBookmarks(await file.text(),'待整理');
   if(!result.rows.length)throw Error('没有可导入的网址，请选择浏览器导出的书签 HTML。');
   setPreview(result);
  }catch(e){setMessage(String(e))}finally{setBusy(false)}
 }
 async function save(){
  if(!preview)return;
  setBusy(true);setMessage('');
  try{const result=await learningCommand<{added:number;skipped:number}>('bookmarks',{rows:preview.rows});
   setPreview(null);await onImported();setMessage(`新增 ${result.added} 条，跳过已有或重复网址 ${result.skipped} 条。`);
  }catch(e){setMessage(String(e))}finally{setBusy(false)}
 }
 return <details className="learning-import" open={empty||undefined}><summary>{'导入自己的浏览器书签（HTML）'}</summary>
 <ol><li>{'在 Chrome 的书签管理器或 Edge 的收藏夹中打开「…」菜单，选择「导出书签」或「导出收藏夹」，保存 HTML 文件。'}</li>
 <li>{'推荐先用 AI 整理。将需要分类的书签 HTML 和下方提示词交给你选择的 AI，然后下载它返回的 HTML。已有分类也可以直接导入。'}
 <details><summary>{'查看 AI 分类提示词'}</summary><p>{prompt}</p><button className="button" onClick={()=>void navigator.clipboard.writeText(prompt).then(()=>setMessage('提示词已复制。')).catch(()=>setMessage('请选中上方文字复制。'))}>{'复制提示词'}</button></details></li>
 <li>{'选择处理后的 HTML，核对分类后确认导入。SciDesk 在本地读取书签文件。'} <input type="file" disabled={busy} accept=".htm,.html,text/html" aria-label={'选择书签 HTML'} onChange={e=>{const f=e.target.files?.[0];e.target.value='';if(f)void choose(f)}}/></li></ol>
 {preview&&<div><p>{'可读取的网址'}: {preview.rows.length} · {'无效网址'}: {preview.invalid}</p><ul>{Array.from(new Set(preview.rows.map(r=>[r.category1,r.category2].filter(Boolean).join(' › ')))).map(name=><li key={name}>{name}</li>)}</ul><button className="button primary" disabled={busy} onClick={()=>void save()}>{'确认导入书签'}</button></div>}
 {message&&<p role="status">{message}</p>}</details>
}
