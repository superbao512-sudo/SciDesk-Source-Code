import {uiText} from '../../locales/language';
import {lazy,Suspense,useCallback,useEffect,useRef,useState} from 'react';
import {Atom,Globe,ImagePlus,MoreHorizontal,Plus,Puzzle,Settings2} from 'lucide-react';
import {useNavigate} from 'react-router-dom';
import {isTauri} from '@tauri-apps/api/core';
import {screenshotCommand} from './screenshot/service';
import {builtinTools,type BuiltinTool} from './builtinTools';
import {Modal} from '../../components/common/Modal';
import {PopupMenu,type MenuAction} from '../projects/PopupMenu';
import {launchTool,readToolIcon,toolService,validToolUrl,type ToolEntry,type ToolInput} from './toolService';
import '../projects/projects.css';
import './tools.css';
const LocalToolForm=lazy(()=>import('./LocalToolForm').then(module=>({default:module.LocalToolForm})));
const ScreenshotSettings=lazy(()=>import('./screenshot/ScreenshotSettings').then(module=>({default:module.ScreenshotSettings})));

function ToolIcon({tool,preview}:{tool?:ToolEntry;preview?:string|null}){
 const icon=preview===undefined?tool?.icon:preview;const [failed,setFailed]=useState(false);
 return <span className={`tool-app-icon${tool?.id==='densityflow'?' tool-densityflow':''}`}>{icon&&!failed?<img src={icon} alt="" onError={()=>setFailed(true)}/>:tool?.kind==='local'?<Puzzle size={35} strokeWidth={1.5}/>:tool?.id==='densityflow'?<Atom size={39} strokeWidth={1.7}/>:<Globe size={35} strokeWidth={1.5}/>}</span>;
}
export function ToolsPage(){
 const [tools,setTools]=useState<ToolEntry[]|null>(null),[error,setError]=useState(''),[form,setForm]=useState<ToolEntry|null|false>(false),[busy,setBusy]=useState(false),[removing,setRemoving]=useState<ToolEntry|null>(null),[menu,setMenu]=useState<{tool:ToolEntry;x:number;y:number}|null>(null);
 const [localForm,setLocalForm]=useState<ToolEntry|null|false>(false);
 const navigate=useNavigate();const [captureSettings,setCaptureSettings]=useState(false);
 const openBuiltin=async(tool:BuiltinTool)=>{setBusy(true);setError('');try{await tool.launch(navigate)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const reload=useCallback(async()=>setTools(await toolService.list()),[]);
 useEffect(()=>{let active=true;void toolService.list().then(rows=>{if(active)setTools(rows)}).catch(e=>{if(active)setError(String(e))});return()=>{active=false}},[]);
 useEffect(()=>{if(!isTauri())return;let active=true;void screenshotCommand<{error:string|null}>('settings').then(value=>{if(active&&value.error)setError(value.error)}).catch(e=>{if(active)setError(String(e))});return()=>{active=false}},[]);
 const open=async(tool:ToolEntry)=>{if(busy)return;setBusy(true);setError('');try{await launchTool(tool)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const actions=(tool:ToolEntry):MenuAction[]=>[{label:'打开工具',run:()=>void open(tool)},{label:tool.kind==='local'?'重新加载工具':'编辑网页工具',run:()=>tool.kind==='local'?setLocalForm(tool):setForm(tool)},{label:'移除工具',danger:true,run:()=>setRemoving(tool)}];
 return <section className="tools-page"><header className="tools-heading"><h1>{uiText("工具")}</h1></header>
  {error&&<p className="projects-error" role="alert">{error}</p>}{!tools&&!error&&<p role="status">{uiText("正在加载工具…")}</p>}
  <div className="tools-app-grid">{builtinTools.map(tool=><article className="tool-app" key={tool.id}><button className="tool-app-launch" disabled={busy} onClick={()=>void openBuiltin(tool)} aria-label={uiText("打开 {0}",uiText(tool.name))}><span className="tool-app-icon"><tool.icon size={35} strokeWidth={1.5}/></span><strong>{uiText(tool.name)}</strong><small>{uiText(tool.description)}</small></button>{tool.settings==='screenshot'&&<button className="tool-more icon-button" aria-label={uiText("高级截图设置")} title={uiText("截图设置")} onClick={()=>setCaptureSettings(true)}><Settings2 size={17}/></button>}</article>)}{tools?.map(tool=><article className="tool-app" key={tool.id} onContextMenu={e=>{e.preventDefault();setMenu({tool,x:e.clientX,y:e.clientY})}}>
   <button className="tool-app-launch" disabled={busy} aria-label={uiText("打开 {0}",tool.name)} title={tool.kind==='local'?tool.local?.root:tool.url} onClick={()=>void open(tool)} onKeyDown={e=>{if(e.key==='ContextMenu'||e.shiftKey&&e.key==='F10'){e.preventDefault();const r=e.currentTarget.getBoundingClientRect();setMenu({tool,x:r.x,y:r.bottom})}}}><ToolIcon key={tool.icon??tool.id} tool={tool}/><strong>{tool.name}</strong><small title={tool.description}>{tool.description}</small>{tool.kind==='local'&&tool.local?.available===false&&<span className="tool-icon-warning">{uiText("工具文件已失效，请重新加载")}</span>}{tool.icon_warning&&<span className="tool-icon-warning">{uiText("图标不可用，可重新上传")}</span>}</button>
   <button className="tool-more icon-button" aria-label={uiText("管理 {0}",tool.name)} title={uiText("管理工具")} onClick={e=>{const r=e.currentTarget.getBoundingClientRect();setMenu({tool,x:r.right-200,y:r.bottom})}}><MoreHorizontal size={17}/></button>
  </article>)}<button className="tool-app-add" onClick={()=>setForm(null)} disabled={busy}><span className="tool-add-icon"><Globe size={29} strokeWidth={1.5}/></span><strong>{uiText("添加网页工具")}</strong></button><button className="tool-app-add" onClick={()=>setLocalForm(null)} disabled={busy}><span className="tool-add-icon"><Plus size={30} strokeWidth={1.5}/></span><strong>{uiText("添加工具")}</strong></button></div>
  {menu&&<PopupMenu {...menu} actions={actions(menu.tool)} onClose={()=>setMenu(null)}/>}
  {form!==false&&<ToolForm key={form?.id??'new'} tool={form} onClose={()=>setForm(false)} onSaved={async()=>{await reload();setForm(false)}}/>}
  {localForm!==false&&<Suspense fallback={<p role="status">{uiText("正在加载工具窗口…")}</p>}><LocalToolForm key={localForm?.id??'local-new'} tool={localForm} onClose={()=>setLocalForm(false)} onSaved={async()=>{await reload();setLocalForm(false)}}/></Suspense>}
  {captureSettings&&<Suspense fallback={null}><ScreenshotSettings onClose={()=>setCaptureSettings(false)}/></Suspense>}
  {removing&&<Modal title={uiText("移除工具")} closeDisabled={busy} onClose={()=>setRemoving(null)} footer={<><button className="button" disabled={busy} onClick={()=>setRemoving(null)}>{uiText("取消")}</button><button className="button project-danger" disabled={busy} onClick={()=>{setBusy(true);setError('');void toolService.remove(removing.id).then(reload).then(()=>setRemoving(null)).catch(e=>setError(String(e))).finally(()=>setBusy(false))}}>{uiText("确认移除")}</button></>}><p>{uiText("从工具页移除「")}{removing.name}{uiText("」？原始工具文件和上传图片不会删除。")}</p>{error&&<p role="alert">{error}</p>}</Modal>}
 </section>
}
function ToolForm({tool,onClose,onSaved}:{tool:ToolEntry|null;onClose:()=>void;onSaved:()=>Promise<void>}){
 const [name,setName]=useState(tool?.name??''),[description,setDescription]=useState(tool?.description??''),[url,setUrl]=useState(tool?.url??''),[icon,setIcon]=useState<string|null|undefined>(undefined),[busy,setBusy]=useState(false),[loadingIcon,setLoadingIcon]=useState(false),[error,setError]=useState('');const input=useRef<HTMLInputElement>(null);
 const upload=async(file?:File)=>{if(!file)return;setLoadingIcon(true);setError('');try{setIcon(await readToolIcon(file))}catch(e){setError(String(e))}finally{setLoadingIcon(false)}};
 const save=async()=>{setError('');if(!validToolUrl(url.trim())){setError(uiText("请输入有效的 HTTP/HTTPS 网址"));return}setBusy(true);try{const values:ToolInput={...(tool?{id:tool.id}:{}),name,description,url,...(icon!==undefined?{icon:icon?.split(',')[1]??null}:{})};await toolService.save(values);await onSaved()}catch(e){setError(String(e))}finally{setBusy(false)}};
 return <Modal title={tool?uiText("编辑网页工具"):uiText("添加网页工具")} className="tool-form-dialog" closeDisabled={busy||loadingIcon} onClose={onClose} footer={<><button className="button" disabled={busy||loadingIcon} onClick={onClose}>{uiText("取消")}</button><button className="button primary" disabled={busy||loadingIcon} form="tool-form" type="submit">{busy?uiText("正在保存…"):uiText("保存")}</button></>}><form id="tool-form" className="tool-form" onSubmit={e=>{e.preventDefault();void save()}}>
  {error&&<p className="projects-error" role="alert">{error}</p>}<div className="tool-form-icon"><ToolIcon key={icon===undefined?tool?.icon??'default':icon??'clear'} tool={tool??undefined} preview={icon===undefined?tool?.icon:icon}/><div><button className="button" type="button" disabled={busy||loadingIcon} onClick={()=>input.current?.click()}><ImagePlus size={16}/>{loadingIcon?uiText("正在读取…"):uiText("上传图标")}</button><input ref={input} type="file" accept="image/png,image/jpeg,image/webp" aria-label={uiText("上传工具图标")} disabled={busy||loadingIcon} onChange={e=>{void upload(e.target.files?.[0]);e.target.value=''}} hidden/><small>{uiText("PNG、JPEG、WebP，最大 2 MB")}</small>{(icon??tool?.icon)&&icon!==null&&<button className="tool-reset-icon" type="button" disabled={busy||loadingIcon} onClick={()=>setIcon(null)}>{uiText("使用默认图标")}</button>}</div></div>
  <label>{uiText("名称")}<input required autoFocus maxLength={80} value={name} onChange={e=>setName(e.target.value)}/></label><label>{uiText("说明")}<textarea rows={2} maxLength={500} value={description} onChange={e=>setDescription(e.target.value)} placeholder={uiText("可选，显示在名称下方")}/></label><label>{uiText("网址")}<input type="url" required maxLength={8192} placeholder="https://" value={url} onChange={e=>setUrl(e.target.value)}/></label>
 </form></Modal>
}
