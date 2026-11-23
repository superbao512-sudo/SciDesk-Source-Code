import {uiText} from '../../locales/language';
import {useEffect,useRef,useState} from 'react';
import {ExternalLink,RefreshCw,Search} from 'lucide-react';
import {dateLabel,errorMessage,journalCommand,openArticle} from './journalService';
import {JournalFeed} from './JournalFeed';
import {searchKeywords,type ProjectPaper} from '../projects/projectLiterature';

type Results={items:ProjectPaper[];warnings:string[]};
type Recommendations=Results&{bound:boolean;interests:string[];updatedAt:string|null;refreshing:boolean};

function Articles({items}:{items:ProjectPaper[]}){
 const [error,setError]=useState('');
 return <>{error&&<p role="alert" className="journal-error">{error}</p>}
  <ol className="discovery-articles">{items.map(paper=><li key={paper.url}>
    <article>
     <h3><button onClick={()=>{setError('');void openArticle(paper.url).catch(e=>setError(errorMessage(e)))}}>{paper.title}<ExternalLink size={13}/></button></h3>
     <p>{paper.authors.slice(0,8).join(', ')}{paper.authors.length>8?uiText(" 等"):''}</p>
     <div className="discovery-source"><span>{paper.journal||(paper.source==='arxiv'?'arXiv':'Crossref')}</span><time>{paper.publishedAt?.slice(0,10)}</time></div>
     <div className="journal-terms">{paper.matchedKeywords.map(term=><span key={term}>{term}</span>)}</div>
    </article>
  </li>)}</ol></>;
}

function SourceWarnings({warnings}:{warnings:string[]}){
 return warnings.length>0?<details className="discovery-warnings"><summary>{uiText("部分来源未能更新 · 查看原因")}</summary>
  {warnings.map(w=><p key={w}>{w}</p>)}</details>:null;
}

function Recommended({active,revision}:{active:boolean;revision:number}){
 const [feed,setFeed]=useState<Recommendations|null>(null);
 const [busy,setBusy]=useState(false),[error,setError]=useState(''),[request,setRequest]=useState(0);
 const manual=useRef(false);
 useEffect(()=>{
  if(!active)return;
  let alive=true,timer:ReturnType<typeof setTimeout>;
  async function load(){
   let delay=60000;
   try{
    const cached=await journalCommand<Recommendations>('recommendations');
    if(!alive)return;
    setFeed(cached);setError('');
    if(cached.interests.length){
     const force=manual.current;manual.current=false;
     setBusy(true);
     const next=await journalCommand<Recommendations>('refresh_recommendations',{force});
     if(!alive)return;
     setFeed(next);
     if(next.refreshing)delay=2500;
    }
   }catch(e){if(alive)setError(errorMessage(e))}
   finally{if(alive){setBusy(false);timer=setTimeout(()=>void load(),delay)}}
  }
  void load();
  return ()=>{alive=false;clearTimeout(timer)};
 },[active,revision,request]);
 const pending=busy||!!feed?.refreshing;
 return <section className="discovery-content" aria-label={uiText("为我推荐")}>
  <header className="discovery-header"><div><h2>{uiText("为我推荐")}</h2>
   {feed?.interests.length?<p>{uiText("根据")}{feed.interests.join(' · ')}</p>:null}</div>
   <button disabled={pending||!feed?.interests.length} onClick={()=>{manual.current=true;setRequest(v=>v+1)}}><RefreshCw size={15} className={pending?'journal-spin':''}/>{pending?uiText("正在更新…"):uiText("立即更新")}</button>
  </header>
  {error&&<p role="alert" className="journal-error">{error}</p>}
  {!feed&&!error&&<p role="status">{uiText("正在读取推荐…")}</p>}
  {feed&&!feed.bound&&<p>{uiText("请先在“我的学术主页”连接 Google Scholar 公开主页。")}</p>}
  {feed?.bound&&!feed.interests.length&&<p>{uiText("学术主页尚无研究方向，请更新主页或使用自定义关键词检索。")}</p>}
  {!!feed?.interests.length&&<>
   <p className="discovery-meta" role="status">{uiText("运行期间每小时更新")}{feed.updatedAt?uiText(" · 上次更新 ")+dateLabel(feed.updatedAt):''}{pending?uiText(" · 正在检索近期论文"):''}</p>
   <SourceWarnings warnings={feed.warnings}/>
   {!pending&&!feed.items.length&&<p>{feed.warnings.length?uiText("暂未取得匹配论文，将自动重试。"):uiText("近期未找到匹配论文，将继续自动更新。")}</p>}
   <Articles items={feed.items}/>
  </>}
 </section>;
}

export function JournalDiscovery({active,revision,onChanged}:{active:boolean;revision:number;onChanged:()=>void}){
 const [mode,setMode]=useState<'recommended'|'custom'>('recommended'),[legacy,setLegacy]=useState(false);
 const [input,setInput]=useState(''),[days,setDays]=useState(365),[busy,setBusy]=useState(false);
 const [found,setFound]=useState<Results|null>(null),[error,setError]=useState(''),[searched,setSearched]=useState('');
 async function search(){
  const keywords=searchKeywords(input);
  if(!keywords.length||keywords.length>8){setError(uiText("请输入 1–8 个关键词或短语"));return}
  setBusy(true);setError('');setFound(null);setSearched(keywords.join(' · '));
  try{setFound(await journalCommand<Results>('keyword_search',{keywords,days}))}catch(e){setError(errorMessage(e))}finally{setBusy(false)}
 }
 return <div className="journal-discovery">
  <nav className="discovery-nav" aria-label={uiText("期刊推送方式")}>
   <button aria-pressed={mode==='recommended'} onClick={()=>setMode('recommended')}>{uiText("为我推荐")}</button>
   <button aria-pressed={mode==='custom'} onClick={()=>setMode('custom')}>{uiText("自定义关键词检索")}</button>
  </nav>
  {mode==='recommended'?<Recommended active={active} revision={revision}/>:<section className="discovery-content" aria-label={uiText("自定义关键词检索")}>
   <h2>{uiText("自定义关键词检索")}</h2>
   <form className="discovery-form" onSubmit={e=>{e.preventDefault();void search()}}>
    <label>{uiText("关键词")}<input required maxLength={980} value={input} disabled={busy} onChange={e=>setInput(e.target.value)} placeholder={uiText("输入关键词，多个短语用逗号分隔")}/></label>
    <label>{uiText("发表时间")}<select value={days} disabled={busy} onChange={e=>setDays(Number(e.target.value))}><option value={30}>{uiText("近 30 天")}</option><option value={365}>{uiText("近一年")}</option><option value={1825}>{uiText("近五年")}</option></select></label>
    <button disabled={busy||!input.trim()} type="submit"><Search size={15}/>{busy?uiText("正在检索…"):uiText("检索论文")}</button>
   </form>
   {error&&<p role="alert" className="journal-error">{error}</p>}
   {found&&<><SourceWarnings warnings={found.warnings}/><p className="discovery-meta" role="status">{searched} · {found.items.length}{uiText("篇匹配论文")}</p><Articles items={found.items}/></>}
   <details className="discovery-legacy" onToggle={e=>setLegacy(e.currentTarget.open)}><summary>{uiText("已保存的主题与推送记录")}</summary>{legacy&&<JournalFeed revision={revision} onChanged={onChanged}/>}</details>
  </section>}
 </div>
}
