import {uiText} from '../../locales/language';
import {useCallback,useEffect,useRef,useState} from 'react';
import {useLocation,useNavigate} from 'react-router-dom';
import {ArrowLeft,BookOpen,ChevronRight,FileText,Folder,MoreHorizontal,RefreshCw,Copy,ClipboardPaste,FolderPlus,ArrowUp,ExternalLink} from 'lucide-react';
import {NameDialog,ConfirmOperation} from '../projects/ProjectDialogs';
import {PopupMenu,type MenuAction} from '../projects/PopupMenu';
import {joinDisplay} from '../projects/projectService';
import {transferSummary,type TransferReport} from '../files/transferReport';
import {useFileRows,useNativeFileDrop,type FileTarget} from '../files/useFileDragDrop';
import {ManualForm,ReconnectForm} from './ManualDialogs';
import {manualCommand,localName,type ManualDetail,type LocalLink,type ManualEntry} from './manualService';

type FolderLocation={linkId:string;relative:string};
const entryKey=(entry:ListedEntry)=>`${entry.link_id}:${entry.relative}`;
type ListedEntry=ManualEntry&{link_id:string};
export function ManualFiles({id,onBack}:{id:string;onBack:()=>void}){
 const navigation=useLocation(),navigate=useNavigate();
 const editor=(linkId:string,base:string,chapter?:string)=>navigate("/tools/manual-editor",{state:{scope:{id,linkId,base},chapter,reading:!!chapter,back:"/manuals/"+id}});
 const [manual,setManual]=useState<ManualDetail|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false),[connecting,setConnecting]=useState<LocalLink|null>(null),[editing,setEditing]=useState(false),[unlink,setUnlink]=useState<LocalLink|null>(null),[menu,setMenu]=useState<{x:number;y:number;link:LocalLink}|null>(null),[location,setLocation]=useState<{linkId:string;relative:string}|null>(null),[entries,setEntries]=useState<ListedEntry[]|null>(null),[revision,setRevision]=useState(0),[managing,setManaging]=useState(!!(navigation.state as {manageLinks?:boolean}|null)?.manageLinks);
 const [fileMenu,setFileMenu]=useState<{x:number;y:number;entry:ListedEntry|null}|null>(null);
 const [change,setChange]=useState<{op:'rename_entry'|'trash_entry';entry:ListedEntry;path:string}|null>(null);
 const [newFolder,setNewFolder]=useState<FolderLocation|null>(null);
 const folders=manual?.links.filter(link=>link.kind==='folder'&&link.status==='available')??[];
 const directory:FolderLocation|null=location??(folders.length===1?{linkId:folders[0].id,relative:''}:null);
 const pathOf=(target:FolderLocation)=>{const link=manual?.links.find(l=>l.id===target.linkId);return link?(target.relative?joinDisplay(link.path,target.relative):link.path):''};
 const ready=!!manual;
 const load=useCallback(async(op='get')=>{const next=await manualCommand<ManualDetail>(op,{id});setManual(next);setRevision(v=>v+1)},[id]);
 useEffect(()=>{let active=true;void manualCommand<ManualDetail>('get',{id}).then(next=>{if(active)setManual(next)}).catch(e=>{if(active)setError(String(e))});return()=>{active=false}},[id]);
 useEffect(()=>{if(!ready||managing)return;let active=true;void manualCommand<{entries:ManualEntry[]}>(location?'browse':'browse_root',{id,...(location?{linkId:location.linkId,relative:location.relative}:{})}).then(next=>{if(active)setEntries(next.entries.map(e=>({...e,link_id:location?.linkId||e.link_id!})))}).catch(e=>{if(active){setEntries([]);setError(`无法读取，请重新扫描或重新连接：${String(e)}`)}});return()=>{active=false}},[id,location,revision,managing,ready]);
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError('');try{await fn()}catch(e){setError(String(e))}finally{setBusy(false)}};
 const open=(target:LocalLink,relative='')=>void act(async()=>{await manualCommand('open',{id,linkId:target.id,relative})});
 const enter=(linkId:string,relative='')=>{rows.setSelected([]);setNotice('');setError('');setEntries(null);setManaging(false);setLocation({linkId,relative})};
 const showRoot=()=>{rows.setSelected([]);setNotice('');setError('');setManaging(false);setLocation(null);setEntries(null);setRevision(v=>v+1)};
 const clickEntry=(entry:ListedEntry)=>{const source=manual?.links.find(l=>l.id===entry.link_id);if(!source)return;if(entry.is_dir)enter(source.id,entry.relative);else if(/\.(md|markdown)$/i.test(entry.name))editor(source.id,"",entry.relative||entry.name);else open(source,entry.relative)};
 const [notice,setNotice]=useState('');const dropArea=useRef<HTMLDivElement>(null);
 const startDrag=useCallback(async(keys:string[])=>{const chosen=(entries??[]).filter(entry=>keys.includes(`${entry.link_id}:${entry.relative}`));try{await manualCommand('drag_files',{id,entries:chosen.map(entry=>({linkId:entry.link_id,relative:entry.relative}))})}finally{await load()}},[entries,id,load]);
 const rows=useFileRows((entries??[]).map(entry=>`${entry.link_id}:${entry.relative}`),startDrag,setError);
 const selection=(entries??[]).filter(entry=>rows.selected.includes(entryKey(entry)));
 const copyFiles=(chosen:ListedEntry[])=>void act(async()=>{
  const count=await manualCommand<number>('copy_files',{id,entries:chosen.map(entry=>({linkId:entry.link_id,relative:entry.relative}))});
  setNotice(`已复制 ${count} 项到系统剪贴板`);
 });
 const pasteFiles=(target:FolderLocation)=>void act(async()=>{
  setNotice(uiText("正在粘贴文件…"));
  try{const report=await manualCommand<TransferReport>('paste_files',{id,...target});setNotice(transferSummary(report,'copy'));await load()}
  catch(e){setNotice('');throw e}
 });
 const copyPath=(path:string)=>void act(async()=>{await navigator.clipboard.writeText(path);setNotice(uiText("路径已复制"))});
 const actionsFor=(entry:ListedEntry|null):MenuAction[]=>{
  const target=entry?{linkId:entry.link_id,relative:entry.relative}:directory;
  const folder=entry?.is_dir?target:directory;
  const chosen=entry&&!selection.some(r=>entryKey(r)===entryKey(entry))?[entry]:selection;
  const actions:MenuAction[]=[];
  if(entry)actions.push({label:'打开',run:()=>clickEntry(entry)},{label:chosen.length>1?`复制选中的 ${chosen.length} 项`:'复制',run:()=>copyFiles(chosen)});
  if(entry&&!entry.is_dir&&/\.(md|markdown)$/i.test(entry.name))actions.push({label:'使用系统程序打开',run:()=>open(manual!.links.find(l=>l.id===entry.link_id)!,entry.relative)});
  if(entry?.is_dir)actions.push({label:'在此编写手册',run:()=>editor(entry.link_id,entry.relative)});
  actions.push({label:entry?.is_dir?'粘贴到此文件夹':'粘贴到当前目录',disabled:!folder,run:()=>folder&&pasteFiles(folder)});
  if(!entry)actions.push({label:'新建文件夹',disabled:!folder,run:()=>setNewFolder(folder)});
  if(target)actions.push({label:entry?'复制路径':'复制当前路径',run:()=>copyPath(pathOf(target))},
   {label:entry?'在文件资源管理器中定位':'打开所在目录',run:()=>void act(async()=>{await manualCommand(entry?'reveal':'open_directory',{id,...target})})});
  if(entry&&target){
   const path=pathOf(target);
   actions.push({label:'重命名',run:()=>setChange({op:'rename_entry',entry,path})},
    {label:'删除（移入回收站）',danger:true,run:()=>setChange({op:'trash_entry',entry,path})});
  }
  return actions;
 };
 const confirmChange=async(name?:string)=>{
  if(!change)return;
  const {op,entry,path}=change;
  const result=await manualCommand<{warning?:string}>(op,{id,linkId:entry.link_id,relative:entry.relative,isDir:entry.is_dir,confirm_path:path,name});
  rows.setSelected([]);setError('');
  setNotice(result.warning||(op==='rename_entry'?`已重命名为「${name}」`:`「${entry.name}」已移入回收站`));
  // Disk operations have completed; a refresh error must not invite a second submission.
  try{await load()}catch(e){setError(`本地操作已完成，刷新失败：${String(e)}`)}
 };
 const dropped=useCallback((paths:string[],target:FileTarget|null,mode:'copy'|'move')=>{
  const folders=(manual?.links??[]).filter(link=>link.kind==='folder'&&link.status==='available');
  const destination=target??(location?{linkId:location.linkId,relative:location.relative}:folders.length===1?{linkId:folders[0].id,relative:''}:null);
  const operation=destination?mode:'link';setNotice(`正在${operation==='link'?'关联':operation==='move'?'移动':'复制'} ${paths.length} 项…`);
  void manualCommand<TransferReport>(destination?'drop_files':'drop_links',{id,linkId:destination?.linkId,relative:destination?.relative,paths,mode,conflict:'keep'}).then(report=>setNotice(transferSummary(report,operation))).catch(e=>setError(String(e))).finally(()=>{void load().catch(e=>setError(String(e)))});
 },[manual,location,id,load]);
 const hover=useNativeFileDrop(dropArea,!!manual&&!managing&&!editing&&!busy&&!change&&!newFolder&&!unlink&&!connecting,dropped,setError);
 useEffect(()=>{const refresh=()=>{void load().catch(e=>setError(String(e)))};window.addEventListener('focus',refresh);return()=>window.removeEventListener('focus',refresh)},[load]);
 if(editing&&manual)return <div className="projects-workspace manuals-workspace"><ManualForm manual={manual} onCancel={()=>setEditing(false)} onManage={()=>{setEditing(false);setManaging(true);setLocation(null)}} onSaved={async()=>{await load();setEditing(false)}}/></div>;
 return <div className="projects-workspace manuals-workspace">
  {error&&<div className="projects-error" role="alert">{error}</div>}
  <section className="project-file-pane" onKeyDown={e=>{
   if(managing||busy||change||newFolder||connecting||unlink||!(e.ctrlKey||e.metaKey)||e.altKey||e.shiftKey||e.target instanceof Element&&e.target.closest('input:not([type=checkbox]),textarea,select,[contenteditable=true]'))return;
   const k=e.key.toLowerCase();
   if(k==='c'&&selection.length){e.preventDefault();if(!e.repeat)copyFiles(selection)}
   if(k==='v'&&directory){e.preventDefault();if(!e.repeat)pasteFiles(directory)}
   if(k==='a'&&entries){e.preventDefault();rows.setSelected(entries.map(entryKey))}
  }} aria-label={managing?uiText("手册关联管理"):uiText("手册本地文件")}>
   <div className="project-file-toolbar"><div className="project-file-history"><button className="button" onClick={onBack}><ArrowLeft size={16}/>{uiText("手册列表")}</button>{managing&&<button className="button" onClick={showRoot}>{uiText("返回内容")}</button>}{!managing&&location&&<button className="icon-button" aria-label={uiText("上一级目录")} onClick={()=>location.relative?enter(location.linkId,location.relative.split('/').slice(0,-1).join('/')):showRoot()}><ArrowUp size={16}/></button>}<strong>{manual?.name||uiText("手册")}{managing?uiText(" · 本地关联"):''}</strong></div><div className="project-file-actions">{!managing&&<>
    <button className="button" disabled={!directory||busy} onClick={()=>directory&&editor(directory.linkId,directory.relative)}><BookOpen size={15}/>{uiText("编写手册")}</button>
    <button className="button" title={uiText("复制所选文件或文件夹（Ctrl+C）")} disabled={busy||!selection.length} onClick={()=>copyFiles(selection)}><Copy size={15}/>{uiText("复制")}</button>
    <button className="button" title={directory?pathOf(directory):uiText("请选择一个本地文件夹")} disabled={busy||!directory} onClick={()=>directory&&pasteFiles(directory)}><ClipboardPaste size={15}/>{uiText("粘贴")}</button>
    <button className="button" disabled={busy||!directory} onClick={()=>setNewFolder(directory)}><FolderPlus size={15}/>{uiText("新建文件夹")}</button>
   </>}<button className="button" disabled={!manual||busy} onClick={()=>void act(async()=>{setEntries(null);await load('scan')})}><RefreshCw size={15}/>{busy?uiText("正在处理…"):uiText("重新扫描")}</button><button className="button" disabled={!manual||busy} onClick={()=>setEditing(true)}>{uiText("编辑手册")}</button></div></div>
   {connecting&&<ReconnectForm key={connecting.id} id={id} link={connecting} onClose={()=>setConnecting(null)} onSaved={async()=>{await load();setConnecting(null);setLocation(null);setEntries(null)}}/>}
   {unlink&&<div className="manual-confirm" role="alert"><span>{uiText("解除「")}{localName(unlink.path)}{uiText("」的关联？保留本地原文件。")}</span><button className="button" disabled={busy} onClick={()=>setUnlink(null)}>{uiText("取消")}</button><button className="button" disabled={busy} onClick={()=>void act(async()=>{await manualCommand('detach',{id,linkId:unlink.id});setUnlink(null);setLocation(null);setEntries(null);await load()})}>{uiText("确认解除关联")}</button></div>}
   {!managing&&<div className="project-breadcrumbs"><button onClick={showRoot}>{uiText("手册内容")}</button>{folders.length>1&&<select aria-label={uiText("选择本地文件夹")} value={location?.linkId??''} onChange={e=>e.target.value?enter(e.target.value):showRoot()}><option value="">{uiText("全部关联（选择文件夹后可粘贴）")}</option>{folders.map(f=><option key={f.id} value={f.id}>{f.path}</option>)}</select>}{folders.length===1&&<button title={folders[0].path} onClick={()=>enter(folders[0].id)}>{localName(folders[0].path)}</button>}{location?.relative.split('/').filter(Boolean).map((part,i,parts)=><span key={i}><ChevronRight size={13}/><button onClick={()=>enter(location.linkId,parts.slice(0,i+1).join('/'))}>{part}</button></span>)}</div>}
   {notice&&<div className="project-notice" role="status">{notice}</div>}
   <div ref={dropArea} tabIndex={managing?undefined:0} onContextMenu={e=>{
    if(managing||busy)return;e.preventDefault();setFileMenu({x:e.clientX,y:e.clientY,entry:null})
   }} className={`project-file-content${hover!==null?' file-drop-active':''}`}>
    {manual?.links.some(l=>l.status!=='available')&&<div className="project-notice">{manual.links.filter(l=>l.status!=='available').map(l=><div className="manual-invalid" key={l.id}><span title={l.message}>{uiText("失效关联：")}{localName(l.path)}{uiText("。可重新连接或解除关联。")}</span><button className="button" disabled={busy} onClick={()=>setConnecting(l)}>{uiText("重新连接")}</button><button className="button" disabled={busy} onClick={()=>setUnlink(l)}>{uiText("解除关联")}</button></div>)}</div>}
    <table className="project-files-table manual-files"><thead><tr>{!managing&&<th className="manual-select"><input type="checkbox" aria-label={uiText("全选当前目录")} disabled={!entries?.length} checked={!!entries?.length&&selection.length===entries.length} ref={el=>{if(el)el.indeterminate=selection.length>0&&selection.length!==(entries?.length??0)}} onChange={e=>rows.setSelected(e.target.checked?(entries??[]).map(entryKey):[])}/></th>}<th>{uiText("名称")}</th><th>{uiText("类型")}</th><th>{managing?uiText("关联状态"):uiText("修改时间")}</th><th>{managing?uiText("操作"):uiText("大小")}</th></tr></thead><tbody>
     {managing?manual?.links.filter(l=>l.status==='available').map(l=><tr key={l.id}><td><button className="manual-entry" title={l.path} onClick={()=>l.kind==='folder'?enter(l.id):open(l)}>{l.kind==='folder'?<Folder size={17} className="folder-icon"/>:<FileText size={17}/>}<span>{localName(l.path)}</span></button></td><td>{l.kind==='folder'?uiText("文件夹"):uiText("文件")}</td><td>{uiText("已连接")}</td><td><button className="icon-button" aria-label={uiText("关联操作 {0}",localName(l.path))} disabled={busy} onClick={e=>{const r=e.currentTarget.getBoundingClientRect();setMenu({x:r.right-210,y:r.bottom,link:l})}}><MoreHorizontal size={17}/></button></td></tr>):entries?.map(entry=>{const source=manual?.links.find(l=>l.id===entry.link_id);return <tr key={entryKey(entry)} tabIndex={0} onContextMenu={e=>{e.preventDefault();e.stopPropagation();if(busy)return;if(!rows.selected.includes(entryKey(entry)))rows.setSelected([entryKey(entry)]);setFileMenu({x:e.clientX,y:e.clientY,entry})}} data-file-drag aria-label={`${entry.is_dir?'文件夹':'文件'} ${entry.name}`} aria-selected={rows.selected.includes(`${entry.link_id}:${entry.relative}`)} className={`${rows.selected.includes(`${entry.link_id}:${entry.relative}`)?'selected ':''}${hover===`${entry.link_id}:${entry.relative}`?'file-drop-folder':''}`} onPointerDown={e=>rows.pointer(`${entry.link_id}:${entry.relative}`,e)} data-file-drop-target={entry.is_dir?JSON.stringify({id:`${entry.link_id}:${entry.relative}`,linkId:entry.link_id,relative:entry.relative,label:source?joinDisplay(source.path,entry.relative):entry.name}):undefined} onKeyDown={e=>{if(e.target===e.currentTarget&&e.key===' '){e.preventDefault();rows.setSelected([entryKey(entry)])}if(e.target===e.currentTarget&&e.key==='Enter')clickEntry(entry)}}><td className="manual-select"><input type="checkbox" aria-label={uiText("选择 {0}",entry.name)} checked={rows.selected.includes(entryKey(entry))} onChange={e=>rows.setSelected(e.target.checked?[...rows.selected,entryKey(entry)]:rows.selected.filter(key=>key!==entryKey(entry)))}/></td><td><button className="manual-entry" title={source?source.kind==='folder'?joinDisplay(source.path,entry.relative):source.path:entry.name} onClick={e=>{if(!e.ctrlKey&&!e.metaKey&&!e.shiftKey&&!rows.wasDragged())clickEntry(entry)}}>{entry.is_dir?<Folder size={17} className="folder-icon"/>:<FileText size={17}/>}<span>{entry.name}</span></button></td><td>{entry.is_dir?uiText("文件夹"):uiText("文件")}</td><td>{entry.modified?new Date(entry.modified*1000).toLocaleString():'—'}</td><td>{entry.size===null?'—':`${entry.size.toLocaleString()} B`}</td></tr>})}
    </tbody></table>
    {!manual||!managing&&entries===null?<div className="project-pane-empty" role="status">{uiText("正在读取本地内容…")}</div>:(managing?!manual.links.length:entries?.length===0)&&<div className="project-pane-empty"><BookOpen size={30}/><p>{uiText("暂无本地内容")}</p><small>{uiText("可返回手册大厅，通过「导入手册」添加本地内容。")}</small></div>}
   </div><div className="project-file-footer"><span>{manual?.category||uiText("未分类")}{manual?.description?` · ${manual.description}`:''}</span>{!managing&&directory&&<button title={pathOf(directory)} onClick={()=>void act(async()=>{await manualCommand('open_directory',{id,...directory})})}><ExternalLink size={14}/>{uiText("打开所在目录")}</button>}<span>{managing?uiText("{0} 项本地关联",manual?.links.length??0):uiText("{0} 项{1}",entries?.length??0,selection.length?` · 已选 ${selection.length} 项`:'')}</span></div>
  </section>
  {change?.op==='rename_entry'&&<NameDialog title={change.entry.is_dir?uiText("重命名本地文件夹"):uiText("重命名本地文件")} label={uiText("新名称")} initial={change.entry.name} detail={change.path} onClose={()=>setChange(null)} onConfirm={name=>confirmChange(name)}/>}
  {change?.op==='trash_entry'&&<ConfirmOperation title={change.entry.is_dir?uiText("删除本地文件夹"):uiText("删除本地文件")} path={change.path} detail={change.entry.is_dir?'确认后，此文件夹及其中的所有文件将移入 Windows 回收站。':'确认后，此本地文件将移入 Windows 回收站。'} label={uiText("移入回收站")} destructive onClose={()=>setChange(null)} onConfirm={()=>confirmChange()}/>}
  {newFolder&&<NameDialog title={uiText("新建本地文件夹")} label={uiText("文件夹名称")} detail={pathOf(newFolder)} onClose={()=>setNewFolder(null)} onConfirm={async name=>{await manualCommand('create_folder',{id,...newFolder,name});await load();setNotice(`已创建文件夹「${name}」`)}}/>}
  {fileMenu&&<PopupMenu x={fileMenu.x} y={fileMenu.y} actions={actionsFor(fileMenu.entry)} onClose={()=>setFileMenu(null)}/>}
  {menu&&<PopupMenu x={menu.x} y={menu.y} onClose={()=>setMenu(null)} actions={[{label:'重新连接',run:()=>setConnecting(menu.link)},{label:'解除关联（保留原文件）',run:()=>setUnlink(menu.link)}]}/>}
 </div>
}
