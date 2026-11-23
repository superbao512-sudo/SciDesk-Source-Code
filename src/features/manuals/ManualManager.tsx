import {uiText} from '../../locales/language';
import {useCallback,useEffect,useState,type CSSProperties} from 'react';
import {useNavigate,useParams} from 'react-router-dom';
import {BookOpen,FolderOpen,MoreHorizontal,Plus} from 'lucide-react';
import {PopupMenu,type MenuAction} from '../projects/PopupMenu';
import {IconPicker} from '../projects/ProjectDialogs';
import {ProjectIcon} from '../projects/ProjectIcons';
import {SortableListHandle} from '../../components/common/SortableListHandle';
import {manualCommand,type Manual} from './manualService';
import '../projects/projects.css';
import './manuals.css';
import {ManualForm,ManualNameForm} from './ManualDialogs';
import {ManualFiles} from './ManualFiles';

export function ManualManager(){
 const {manualId}=useParams();const navigate=useNavigate();
 const [manuals,setManuals]=useState<Manual[]|null>(null),[error,setError]=useState(''),[form,setForm]=useState<Manual|null|false>(false),[importing,setImporting]=useState(false),[deleting,setDeleting]=useState<Manual|null>(null),[busy,setBusy]=useState(false),[menu,setMenu]=useState<{x:number;y:number;manual:Manual}|null>(null),[iconManual,setIconManual]=useState<Manual|null>(null),[renaming,setRenaming]=useState<Manual|null>(null),[dropTarget,setDropTarget]=useState('');
 const reload=useCallback(async()=>{setManuals(await manualCommand<Manual[]>('list'));setError('')},[]);
 useEffect(()=>{if(manualId)return;let active=true;void manualCommand<Manual[]>('list').then(rows=>{if(active){setManuals(rows);setError('')}}).catch(e=>{if(active)setError(String(e))});return()=>{active=false}},[manualId]);
 const remove=async()=>{if(!deleting)return;setBusy(true);try{await manualCommand('delete',{id:deleting.id});setDeleting(null);await reload()}catch(e){setError(String(e))}finally{setBusy(false)}};
 const mutate=async(op:string,args:Record<string,unknown>)=>{await manualCommand(op,args);await reload()};
 const enterManual=async(manual:Manual)=>{if(busy)return;setBusy(true);setError('');try{const result=await manualCommand<{opened:boolean;markdownLink?:string;chapter?:string}>('enter',{id:manual.id});if(result.markdownLink)navigate("/tools/manual-editor",{state:{scope:{id:manual.id,linkId:result.markdownLink},chapter:result.chapter,reading:true,back:"/manuals"}});else if(!result.opened)navigate(`/manuals/${manual.id}`)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const reorder=async(source:string,target:string)=>{setDropTarget('');if(!manuals||source===target)return;const ids=manuals.map(m=>m.id),from=ids.indexOf(source),to=ids.indexOf(target);if(from<0||to<0)return;ids.splice(to,0,...ids.splice(from,1));try{await mutate('reorder',{ids})}catch(e){setError(String(e))}};
 const move=(manual:Manual,delta:number)=>{const index=manuals?.findIndex(m=>m.id===manual.id)??-1,target=manuals?.[index+delta];if(target)void reorder(manual.id,target.id)};
 const actions=(manual:Manual):MenuAction[]=>[{label:'浏览本地文件',run:()=>navigate(`/manuals/${manual.id}`)},{label:'图标与颜色',run:()=>setIconManual(manual)},{label:'编辑手册信息',run:()=>{setImporting(false);setForm(manual)}},{label:'重命名手册',run:()=>setRenaming(manual)},{label:'管理本地关联',run:()=>navigate(`/manuals/${manual.id}`,{state:{manageLinks:true}})},{label:'向上移动',disabled:manuals?.[0]?.id===manual.id,run:()=>move(manual,-1)},{label:'向下移动',disabled:manuals?.at(-1)?.id===manual.id,run:()=>move(manual,1)},{label:'删除手册（保留本地文件）',danger:true,run:()=>setDeleting(manual)}];
 if(manualId)return <ManualFiles key={manualId} id={manualId} onBack={()=>navigate('/manuals')}/>;
 return <div className="projects-workspace manuals-workspace">
  {error&&<div className="projects-error" role="alert">{error}</div>}
  {form!==false?<ManualForm key={importing?'import':form?.id||'create'} manual={form} importing={importing} manuals={manuals??[]} onCancel={()=>setForm(false)} onSaved={async()=>{await reload();setForm(false)}}/>:renaming?<ManualNameForm manual={renaming} onCancel={()=>setRenaming(null)} onSaved={async()=>{await reload();setRenaming(null)}}/>:<section className="project-list-pane" aria-label={uiText("手册列表")} style={{'--project-name-size':`${manuals?.[0]?.name_font_size??18}px`} as CSSProperties}>
   {deleting&&<div className="manual-confirm" role="alert"><span>{uiText("删除手册「")}{deleting.name}{uiText("」及关联索引？本地原始文件不会删除。")}</span><button className="button" disabled={busy} onClick={()=>setDeleting(null)}>{uiText("取消")}</button><button className="button project-danger" disabled={busy} onClick={()=>void remove()}>{uiText("确认删除手册")}</button></div>}
   <div className="project-list-rows">{manuals?.map(manual=><div key={manual.id} data-manual-id={manual.id} className={`project-list-row${dropTarget===manual.id?' drop-target':''}`} style={{'--project-color':manual.manual_color} as CSSProperties} role="button" tabIndex={0} aria-label={uiText("打开手册 {0}",manual.name)} onClick={()=>void enterManual(manual)} onKeyDown={e=>{if(e.target===e.currentTarget&&(e.key==='Enter'||e.key===' ')){e.preventDefault();void enterManual(manual)}}} onContextMenu={e=>{e.preventDefault();setMenu({x:e.clientX,y:e.clientY,manual})}}>
    <SortableListHandle id={manual.id} name={manual.name} idAttribute="data-manual-id" onTarget={setDropTarget} onReorder={(source,target)=>void reorder(source,target)} onMove={delta=>move(manual,delta)}/>
    <div className="project-row-copy"><div className="project-row-heading"><button className="project-row-icon" title={uiText("设置图标与颜色")} aria-label={uiText("图标设置 {0}",manual.name)} onClick={e=>{e.stopPropagation();setIconManual(manual)}}><ProjectIcon name={manual.icon_name} color={manual.manual_color}/></button><strong title={manual.name}>{manual.name}</strong></div><small className="project-row-description" title={manual.description}>{manual.description}</small></div>
    <span className="project-row-state"><span>{manual.category||uiText("未分类")}</span><small>{manual.links?uiText("已关联 {0} 项",manual.links):uiText("未关联本地")}{manual.invalid?uiText(" · {0} 项失效",manual.invalid):''}</small></span>
    <button className="icon-button" aria-label={uiText("更多操作 {0}",manual.name)} onClick={e=>{e.stopPropagation();const r=e.currentTarget.getBoundingClientRect();setMenu({x:r.right-210,y:r.bottom,manual})}}><MoreHorizontal size={18}/></button>
   </div>)}{manuals&&!manuals.length&&<div className="project-pane-empty"><BookOpen size={30}/><p>{uiText("还没有手册")}</p><small>{uiText("新建手册，或导入本地文件与文件夹。")}</small></div>}{!manuals&&!error&&<div className="project-pane-empty" role="status">{uiText("正在加载手册…")}</div>}</div>
   <div className="project-list-bottom"><button className="button primary" disabled={!manuals} onClick={()=>{setImporting(false);setForm(null)}}><Plus size={17}/>{uiText("新建手册")}</button><button className="button" disabled={!manuals} onClick={()=>{setImporting(true);setForm(null)}}><FolderOpen size={17}/>{uiText("导入手册")}</button></div>
  </section>}
  {menu&&<PopupMenu x={menu.x} y={menu.y} onClose={()=>setMenu(null)} actions={actions(menu.manual)}/>}
  {iconManual&&<IconPicker title={uiText("手册图标")} name={iconManual.icon_name} color={iconManual.manual_color} onClose={()=>setIconManual(null)} onConfirm={(icon_name,manual_color)=>mutate('set_appearance',{id:iconManual.id,icon_name,manual_color})}/>}
 </div>
}

