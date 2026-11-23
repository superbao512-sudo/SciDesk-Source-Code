import {uiText} from '../../locales/language';
import {useMemo,useState} from 'react';
import {ArrowLeft,Atom,BookOpen,ChevronDown,ChevronRight,Code2,ExternalLink,FolderOpen,GraduationCap,Paperclip,Search,X} from 'lucide-react';
import {filterResources,learningDirectory,type Backup,type LearningState,type Resource} from './learningService';

import {purposes,resourcePurpose,resourceTopics,topicIndex,topics,type Topic} from './resourceTopics';

const subjectIcons:Record<string,typeof BookOpen>={
 '材料物理与第一性原理':Atom,'科研计算与数据分析':Code2,'课程、讲座与交流':GraduationCap,'写作与科研服务':BookOpen,
};

type ResourceActions={
 busy:boolean;onOpen:(r:Resource)=>void;onEdit:(r:Resource)=>void;onBackups:(r:Resource)=>void;
};
type Props=ResourceActions&{data:LearningState};

export function LearningLibrary({data,...actions}:Props){
 const [range,setRange]=useState('core'),[query,setQuery]=useState(''),[category,setCategory]=useState<string|null>(null);
 const [topic,setTopic]=useState<Topic|null>(null);
 const [sub,setSub]=useState(''),[property,setProperty]=useState('');
 const visible=useMemo(()=>data.resources.filter(r=>range==='all'||r.tier===range),[data.resources,range]);
 const directory=useMemo(()=>learningDirectory(data.categories,visible),[data.categories,visible]);
 const index=useMemo(()=>topicIndex(visible),[visible]);
 const searching=Boolean(query.trim());
 const activeTopic=searching?null:topic;
 const inDirectory=!searching&&category===null&&!activeTopic;
 const current=searching||activeTopic?'':category??'';
 const base=useMemo(()=>activeTopic?index.get(activeTopic)!:filterResources(visible,query,current,'','all',''),[activeTopic,index,visible,query,current]);
 const related=useMemo(()=>new Set(base.flatMap(resourceTopics)),[base]);
 const rows=useMemo(()=>base.filter(r=>(!sub||r.category2===sub)&&(!property||r.property===property)),[base,sub,property]);
 const parent=data.categories.find(c=>!c.parent_id&&c.name===current);
 const children=parent?data.categories.filter(c=>c.parent_id===parent.id&&base.some(r=>r.category2===c.name)):[];
 const properties=Array.from(new Set(base.map(r=>r.property).filter(Boolean)));
 const backups=useMemo(()=>{const map=new Map<string,Backup[]>();for(const b of data.backups){const list=map.get(b.resource_id)??[];list.push(b);map.set(b.resource_id,list)}return map},[data.backups]);
 const duplicates=useMemo(()=>{const map=new Map<string,number>();for(const r of data.resources)map.set(r.url,(map.get(r.url)??0)+1);return map},[data.resources]);
 function resetFilters(){setSub('');setProperty('')}
 function selectTopic(next:Topic){setTopic(next);setQuery('');resetFilters()}
 function navigate(next:string|null){setTopic(null);setCategory(next);setQuery('');resetFilters()}
 function search(value:string){setQuery(value);resetFilters()}

 return <div className="lr-library">
  <div className="lr-toolbar">
   <label className="lr-search"><Search size={16}/><input type="search" aria-label={uiText("搜索全部学习资源")} placeholder={uiText("在全部分类中搜索标题、网址或关键词")} value={query} onChange={e=>search(e.target.value)}/></label>
   <select aria-label={uiText("资源范围")} value={range} onChange={e=>{setRange(e.target.value);resetFilters()}}>
    <option value="core">{uiText("核心资源")}</option><option value="all">{uiText("核心和可选资源")}</option><option value="optional">{uiText("仅可选资源")}</option>
   </select>
  </div>
  {inDirectory?<div className="lr-scroll">
   <TopicLinks options={topics.filter(t=>index.get(t)!.length)} onSelect={selectTopic}/>
   <div className="lr-section-heading"><h2>{uiText("分类目录")}</h2><button className="learning-text-button" onClick={()=>navigate('')}>{uiText("查看全部")}{visible.length}{uiText("条")}<ChevronRight size={14}/></button></div>
   <div className="lr-directory">{directory.map(group=>{const Icon=subjectIcons[group.name]??FolderOpen;return <section key={group.name} className="lr-subject">
    <h3><Icon size={18}/>{group.name}</h3><ul>{group.entries.map(entry=><li key={entry.name}><button onClick={()=>navigate(entry.name)}><span>{entry.name||uiText("未分类")}</span><small>{entry.count}</small><ChevronRight size={13}/></button></li>)}</ul>
   </section>})}</div>
   {!directory.length&&<p className="learning-empty">{uiText("此范围暂无资源，可切换上方的资源范围。")}</p>}
  </div>:<>
   <div className="lr-results-head"><button className="learning-text-button" onClick={()=>navigate(null)}><ArrowLeft size={14}/>{uiText("分类目录")}</button>
    {activeTopic&&category&&<button className="learning-text-button lr-parent" onClick={()=>navigate(category)}>{category}</button>}
    <div className="lr-section-heading"><h2>{searching?uiText("搜索结果"):activeTopic||current||uiText("全部资源")}</h2><span>{rows.length}{uiText("条资源")}</span></div>
    {current&&<TopicLinks options={topics.filter(t=>related.has(t))} onSelect={selectTopic}/>}
    {!activeTopic&&(children.length>0||properties.length>1||property)&&<div className="lr-facets">
     {children.length>0&&<select aria-label={uiText("二级分类")} value={sub} onChange={e=>setSub(e.target.value)}><option value="">{uiText("全部二级分类")}</option>{children.map(c=><option key={c.id}>{c.name}</option>)}</select>}
     {(properties.length>1||property)&&<select aria-label={uiText("资源属性筛选")} value={property} onChange={e=>setProperty(e.target.value)}><option value="">{uiText("全部资源属性")}</option>{properties.map(p=><option key={p}>{p}</option>)}</select>}
     {(sub||property)&&<button className="learning-text-button" onClick={resetFilters}><X size={13}/>{uiText("清除筛选")}</button>}
    </div>}
   </div>
   {activeTopic?<TopicResources key={JSON.stringify([activeTopic,range])} rows={rows} backups={backups} duplicates={duplicates} {...actions}/>:<ResourceList key={JSON.stringify([category,query,range,sub,property])} rows={rows} showParent={!current} backups={backups} duplicates={duplicates} {...actions}/>}
  </>}
 </div>;
}

function TopicLinks({options,onSelect}:{options:Topic[];onSelect:(t:Topic)=>void}){
 if(!options.length)return null;
 return <nav className="lr-topic-links" aria-label={uiText("跨分类专题")}><span>{uiText("跨分类专题")}</span>{options.map(t=><button key={t} onClick={()=>onSelect(t)}>{t}<ChevronRight size={13}/></button>)}</nav>;
}

type ListProps=ResourceActions&{rows:Resource[];backups:Map<string,Backup[]>;duplicates:Map<string,number>};

function TopicResources({rows,...props}:ListProps){
 const groups=useMemo(()=>{const groups=new Map<string,Resource[]>(purposes.map(p=>[p,[]]));
  for(const row of rows)groups.get(resourcePurpose(row))!.push(row);
  return groups},[rows]);
 return <div className="lr-scroll lr-purpose-list">{purposes.map(purpose=>{
  const entries=groups.get(purpose)!;
  return entries.length>0&&<PurposeSection key={purpose} name={purpose} rows={entries} {...props}/>;
 })}{!rows.length&&<p className="learning-empty">{uiText("当前资源范围内暂无该专题的书签。")}</p>}</div>;
}

function PurposeSection({name,rows,...props}:ListProps&{name:string}){
 const [more,setMore]=useState(false);
 return <section className="lr-purpose"><header><h3>{name}<small>{rows.length}</small></h3>
  {rows.length>2&&<button className="learning-text-button" aria-label={more?uiText("收起{0}",name):uiText("展开全部{0}",name)} aria-expanded={more} onClick={()=>setMore(!more)}>{more?uiText("收起"):uiText("展开全部 {0} 条",rows.length)}</button>}</header>
  {(more?rows:rows.slice(0,2)).map(r=><ResourceRow key={r.id} r={r} showParent={false} {...props}/>)}
 </section>;
}

function ResourceList({rows,showParent,backups,duplicates,busy,onOpen,onEdit,onBackups}:ResourceActions&{
 rows:Resource[];showParent:boolean;backups:Map<string,Backup[]>;duplicates:Map<string,number>;
}){
 const [page,setPage]=useState(0);
 const size=20,total=Math.ceil(rows.length/size),index=Math.min(page,Math.max(0,total-1));
 return <>
  <div className="lr-scroll lr-rows" key={index}>
   {rows.slice(index*size,(index+1)*size).map(r=><ResourceRow key={r.id} r={r} showParent={showParent} backups={backups} duplicates={duplicates} busy={busy} onOpen={onOpen} onEdit={onEdit} onBackups={onBackups}/>)}
   {!rows.length&&<p className="learning-empty">{uiText("未找到符合条件的资源，请调整关键词或筛选条件。")}</p>}
  </div>
  {total>1&&<nav className="lr-pager" aria-label={uiText("学习资源分页")}><span>{index*size+1}–{Math.min((index+1)*size,rows.length)} / {rows.length}</span>
   <button className="button" disabled={!index} onClick={()=>setPage(index-1)}>{uiText("上一页")}</button><button className="button" disabled={index+1>=total} onClick={()=>setPage(index+1)}>{uiText("下一页")}</button>
  </nav>}
 </>;
}

function ResourceRow({r,showParent,backups,duplicates,busy,onOpen,onEdit,onBackups}:Omit<ListProps,'rows'>&{r:Resource;showParent:boolean}){
 const [open,setOpen]=useState(false);
 const files=backups.get(r.id)??[],same=duplicates.get(r.url)??1;
    let site=r.url;try{site=new URL(r.url).hostname.replace(/^www\./,'')}catch{/* Show the stored address for legacy entries. */}
    return <article key={r.id} className="lr-row">
     <div className="lr-row-top"><div className="lr-row-main">
      <button className="learning-title" disabled={busy} onClick={()=>onOpen(r)}>{r.title}<ExternalLink size={12}/></button>
      <div className="lr-row-meta">{[site,showParent?r.category1:'',r.category2,r.tier==='optional'?'可选':''].filter(Boolean).join(' · ')}</div>
     </div><button className="lr-more" aria-label={uiText("资源详情：{0}",r.title)} aria-expanded={open} onClick={()=>setOpen(!open)}>{uiText("详情")}<ChevronDown size={14}/></button></div>
     {open&&<div className="lr-details">
      <p>{r.url}</p>{r.property&&<p>{uiText("资源属性 ·")}{r.property}</p>}{r.tags&&<p>{uiText("标签 ·")}{r.tags}</p>}{r.notes&&<p>{uiText("备注 ·")}{r.notes}</p>}
      {r.origin==='manual'&&<p>{uiText("手动添加")}</p>}{(r.duplicate||same>1)&&<p>{r.duplicate||uiText("相同网址共 {0} 条",same)}</p>}
      <div><button className="button" onClick={()=>onEdit(r)}>{uiText("编辑资源")}</button><button className="button" onClick={()=>onBackups(r)}><Paperclip size={13}/>{files.length?uiText("本地备份（{0}）",files.length):uiText("关联本地备份")}</button>{files.some(b=>!b.available)&&<small>{uiText("有备份需要重新关联")}</small>}</div>
     </div>}
    </article>;
}
