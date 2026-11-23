import {micromark} from 'micromark';
import {gfm,gfmHtml} from 'micromark-extension-gfm';
import {math,mathHtml} from 'micromark-extension-math';
import {manualCommand} from '../manualService';
import paperStyle from './paper.css?inline';

export type Scope={id:string;linkId:string;base?:string};
export type Book={title:string;root:string;chapters:string[];single:boolean};
export type Source={text:string;hash:string};
export type Issue={chapter:string;message:string};
export type Page={html:string;title:string;issues:Issue[]};
export function bookCommand<T>(scope:Scope,op:string,args:Record<string,unknown>={}){
 return manualCommand<T>('book_'+op,{...scope,...args});
}
const escape=(s:string)=>s.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
export const fileTitle=(name:string)=>name.split('/').at(-1)!.replace(/\.(md|markdown)$/i,'');
function localPath(chapter:string,ref:string){
 if(!ref)return chapter;
 if(/^[a-z][a-z\d+.-]*:|^[\\/]|\\/i.test(ref))throw Error('需要手册内的相对路径');
 const parts=chapter.split('/').slice(0,-1);
 for(const part of ref.split('/')){
  if(part==='..'){if(!parts.length)throw Error('路径超出手册目录');parts.pop()}
  else if(part&&part!=='.')parts.push(part);
 }
 return parts.join('/');
}
const headingSlug=(s:string)=>s.trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu,'').replace(/\s+/g,'-');
export async function renderChapter(scope:Scope,chapters:string[],chapter:string,source:string,assets=new Map<string,Promise<string>>()){
 const template=document.createElement('template');
 template.innerHTML=micromark(source,{extensions:[gfm(),math()],htmlExtensions:[gfmHtml(),mathHtml({output:'mathml',trust:false,throwOnError:false})]});
 const body=template.content, issues:Issue[]=[];
 const report=(message:string)=>issues.push({chapter,message});
 const prefix='ch-'+chapters.indexOf(chapter), used=new Set<string>();
 for(const heading of body.querySelectorAll('h1,h2,h3,h4,h5,h6')){
  const slug=headingSlug(heading.textContent||'')||'section';let unique=slug,i=0;
  while(used.has(unique))unique=slug+'-'+(++i);used.add(unique);heading.id=prefix+'-'+unique;
 }
 for(const error of body.querySelectorAll('.katex-error'))report('公式：'+(error.getAttribute('title')||error.textContent));
 for(const img of body.querySelectorAll('img')){
  const src=img.getAttribute('src')||'';img.removeAttribute('src');
  try{
   const path=localPath(chapter,decodeURIComponent(src));
   let data=assets.get(path);if(!data){data=bookCommand<string>(scope,'image',{path});assets.set(path,data)}
   img.src=await data;
  }catch{report('无法嵌入图片：'+src);const label=document.createElement('span');label.className='book-missing';label.textContent='[图片：'+(img.alt||src)+']';img.replaceWith(label)}
 }
 for(const a of body.querySelectorAll<HTMLAnchorElement>('a[href]')){
  const href=a.getAttribute('href')||'';
  if(/^(https?:\/\/|mailto:)/i.test(href)){a.target='_blank';a.rel='noopener noreferrer';continue}
  if(/^#pdf\/[a-f0-9]{64}\/\d+$/.test(href))continue;
  const hash=href.indexOf('#');
  try{
   const path=localPath(chapter,decodeURIComponent(hash<0?href:href.slice(0,hash)));
   const index=chapters.indexOf(path);
   if(index<0)throw Error();
   const fragment=hash>=0?decodeURIComponent(href.slice(hash+1)):'';
   a.href='#ch-'+index+(fragment?'-'+headingSlug(fragment):'');a.dataset.chapter=path;
  }catch{a.removeAttribute('href');report('链接未编入手册：'+href)}
 }
 return {html:template.innerHTML,title:body.querySelector('h1')?.textContent||fileTitle(chapter),issues};
}
export async function compileBook(scope:Scope,book:Book,onChapter:(current:number,total:number,chapter:string)=>void){
 const pages:Page[]=[],assets=new Map<string,Promise<string>>();let size=0;
 for(const chapter of book.chapters){
  onChapter(pages.length+1,book.chapters.length,chapter);
  let page:Page;
  try{
   const source=await bookCommand<Source>(scope,'read',{chapter});
   page=await renderChapter(scope,book.chapters,chapter,source.text,assets);
  }catch(error){throw new Error(chapter+'：'+String(error))}
  size+=page.html.length;if(size>60*1024*1024)throw Error('整本排版内容超过 60 MB，请缩小插图');
  pages.push(page);
 }
 const links=pages.map((p,i)=>'<li><a href="#ch-'+i+'">'+escape(p.title)+'</a></li>').join('');
 const html='<header class="book-cover"><h1>'+escape(book.title)+'</h1><nav aria-label="目录"><h2>目录</h2><ol>'+links+'</ol></nav></header>'+pages.map((p,i)=>'<section class="book-chapter" id="ch-'+i+'">'+p.html+'</section>').join('');
 const template=document.createElement('template');template.innerHTML=html;
 const ids=new Set(Array.from(template.content.querySelectorAll('[id]'),n=>n.id));
 const issues=pages.flatMap(p=>p.issues);
 template.content.querySelectorAll('.book-chapter').forEach((section,i)=>section.querySelectorAll('a[href^="#"]').forEach(a=>{
  if(!a.getAttribute('href')!.startsWith('#pdf/')&&!ids.has(a.getAttribute('href')!.slice(1)))issues.push({chapter:book.chapters[i],message:'找不到链接标题：'+a.textContent});
 }));
 return {html,issues};
}
export function standalone(title:string,html:string){
 return '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>'+escape(title)+'</title><style>'+paperStyle+'</style><main class="book-paper">'+html+'</main></html>';
}
