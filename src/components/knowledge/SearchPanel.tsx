import {uiText} from '../../locales/language';
import {useEffect,useState} from 'react';
import {Link} from 'react-router-dom';
import {Folder,Lightbulb,BookOpen,FileText,ListTodo,Database,ChartNoAxesCombined} from 'lucide-react';
import {objectUrl,SearchService,type SearchResult} from '../../services/search';
import {userError} from '../../locales/zh-CN';

export function SearchPanel({query,immediate,onNavigate}:{query:string;immediate:boolean;onNavigate:()=>void}){
 const [result,setResult]=useState<{query:string;rows:SearchResult[];error:string}|null>(null);
 useEffect(()=>{
  let active=true;
  const timer=window.setTimeout(()=>{
   void SearchService.search(query).then(rows=>{if(active)setResult({query,rows,error:''})}).catch(reason=>{if(active)setResult({query,rows:[],error:userError(reason)})});
  },immediate?0:250);
  return()=>{active=false;window.clearTimeout(timer)};
 },[query,immediate]);
 const current=result?.query===query?result:null;
 const groups:{types:SearchResult['object_type'][];label:string;icon:typeof Folder}[]=[{types:['project'],label:'项目',icon:Folder},{types:['idea'],label:'想法',icon:Lightbulb},{types:['paper'],label:'文献',icon:BookOpen},{types:['manual'],label:'学习',icon:BookOpen},{types:['note'],label:'笔记',icon:FileText},{types:['task'],label:'任务',icon:ListTodo},{types:['dataset','calculation'],label:'数据',icon:Database},{types:['figure'],label:'图表',icon:ChartNoAxesCombined},{types:['manuscript','section'],label:'写作',icon:FileText}];
 return <section id="home-search-results" className="home-search-results" aria-label={uiText("搜索结果")} aria-live="polite" aria-busy={!current}>
  {!current?<p role="status">{uiText("正在搜索本地内容…")}</p>:current.error?<p className="form-error" role="alert">{current.error}</p>:!current.rows.length?<p>{uiText("没有找到与“")}{query}{uiText("”匹配的内容。")}</p>:<><p className="muted">“{query}” · {current.rows.length}{uiText("条结果")}</p>{groups.map(group=>{const rows=current.rows.filter(row=>group.types.includes(row.object_type));return rows.length?<section key={group.label} aria-label={group.label}><p className="eyebrow">{group.label} · {rows.length}</p>{rows.map(row=><Link className="home-search-result" key={row.object_type+'-'+row.id} to={objectUrl(row.object_type,row.id,row.project_id,row.parent_id)} onClick={onNavigate}><group.icon size={17}/><span>{row.title}{row.archived===1&&<em className="tag">{uiText("已归档")}</em>}<small>{row.match_location==='title'?uiText("标题"):uiText("内容")}{uiText("匹配 ·")}{new Date(row.updated_at).toLocaleDateString('zh-CN')} · {row.project_name||uiText("全局知识库")}</small><small>{row.excerpt}</small></span></Link>)}</section>:null})}</>}
 </section>;
}
