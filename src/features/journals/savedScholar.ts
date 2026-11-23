import type {ScholarProfile} from './types';

export function readSavedScholar(html:string){
 const template=document.createElement('template');
 template.innerHTML=html;
 const doc=template.content;
 const text=(node:Element|null)=>node?.textContent?.replace(/\s+/g,' ').trim()||'';
 const name=text(doc.querySelector('#gsc_prf_in'));
 if(!name)throw Error('没有找到研究人员姓名。请保存显示姓名和论文列表的 Scholar 个人主页。');
 const ids=new Set<string>();
 const valid=(id:string|null)=>{if(id&&/^[\w-]{12}$/.test(id))ids.add(id)};
 function identify(raw:string|null,article=false){
  if(!raw)return;
  let url:URL;try{url=new URL(raw,'https://scholar.google.com')}catch{return}
  if(!/^scholar\.google\.(com|[a-z]{2}|(?:co|com)\.[a-z]{2})$/i.test(url.hostname)||url.pathname!=='/citations')return;
  valid(url.searchParams.get('user'));
  if(article)valid(url.searchParams.get('citation_for_view')?.split(':')[0]??null);
 }
 identify(html.match(/<!--\s*saved from url=\(\d+\)(https?:\/\/[^\s<>]+)\s*-->/i)?.[1]?.replaceAll('&amp;','&')??null);
 identify(doc.querySelector('link[rel~="canonical"]')?.getAttribute('href')??null);
 identify(doc.querySelector('meta[property="og:url"]')?.getAttribute('content')??null);
 identify(doc.querySelector('#gsc_prf_pup-img')?.getAttribute('src')??null);
 const works:NonNullable<ScholarProfile['snapshot']>['works']=[];
 for(const row of doc.querySelectorAll('.gsc_a_tr')){
  const link=row.querySelector('.gsc_a_at');if(!text(link))continue;
  identify(link?.getAttribute('href')??null,true);identify(link?.getAttribute('data-href')??null,true);
  const info=row.querySelectorAll('.gs_gray'),year=Number(text(row.querySelector('.gsc_a_y')));
  works.push({title:text(link),authors:text(info[0]),publication:text(info[1]),year:year>=1600&&year<=9999?year:null});
 }
 if(ids.size!==1)throw Error('无法唯一确认此文件的 Scholar ID。请在浏览器打开作者个人主页，再按 Ctrl+S 保存为 HTML。');
 const interests=[...new Set(Array.from(doc.querySelectorAll('.gsc_prf_inta'),text).filter(Boolean))];
 return {url:'https://scholar.google.com/citations?user='+[...ids][0],snapshot:{name,affiliation:text(doc.querySelector('.gsc_prf_il:not([id])')),interests,works}};
}
