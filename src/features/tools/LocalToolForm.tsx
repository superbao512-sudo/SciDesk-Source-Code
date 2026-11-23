import {uiText} from '../../locales/language';
import {useState} from 'react';
import {FolderOpen,Puzzle} from 'lucide-react';
import {Modal} from '../../components/common/Modal';
import {toolService,type ToolEntry,type LocalToolPreview} from './toolService';

const example={schema_version:1,id:'my.research.tool',name:'我的小工具',version:'1.0.0',description:'工具的简短说明',runtime:'exe',entry:'MyTool.exe',args:[],icon:'icon.png'};
export function LocalToolForm({tool,onClose,onSaved}:{tool:ToolEntry|null;onClose:()=>void;onSaved:()=>Promise<void>}){
 const [root,setRoot]=useState(tool?.local?.root??''),[preview,setPreview]=useState<LocalToolPreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const read=async(path:string)=>{setPreview(null);setError('');const result=await toolService.previewLocal(path);setRoot(result.root);setPreview(result)};
 const choose=async()=>{setBusy(true);setError('');try{const path=await toolService.pickLocal();if(path)await read(path)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const load=async()=>{setBusy(true);try{await read(root)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const save=async()=>{if(!preview)return;setBusy(true);setError('');try{await toolService.registerLocal(preview.root,preview.token);await onSaved()}catch(e){setError(String(e))}finally{setBusy(false)}};
 return <Modal title={tool?uiText("重新加载工具"):uiText("添加工具")} className="tool-form-dialog local-tool-dialog" closeDisabled={busy} onClose={onClose} footer={<><button className="button" disabled={busy} onClick={onClose}>{uiText("取消")}</button><button className="button primary" disabled={busy||!preview} onClick={()=>void save()}>{busy?uiText("正在处理…"):preview?.existing_id?uiText("更新工具"):uiText("添加到工具页")}</button></>}><div className="tool-form local-tool-form">
  <p className="local-tool-help">{uiText("选择包含 scidesk-tool.json 的工具文件夹。添加只记录位置，点击图标时才启动。")}</p>
  {error&&<p className="projects-error" role="alert">{error}</p>}
  <label>{uiText("工具文件夹")}<div className="local-tool-directory"><input value={root} disabled={busy} onChange={e=>{setRoot(e.target.value);setPreview(null);setError('')}} placeholder={uiText("选择或粘贴工具文件夹的完整路径")}/><button type="button" className="button" disabled={busy} onClick={()=>void choose()}><FolderOpen size={16}/>{uiText("选择文件夹")}</button></div></label>
  <button className="button local-tool-read" disabled={busy||!root.trim()} onClick={()=>void load()}>{uiText("读取工具")}</button>
  {preview&&<section className="local-tool-preview" aria-label={uiText("工具预览")}><div className="local-tool-identity"><span className="tool-app-icon">{preview.icon?<img src={preview.icon} alt={uiText("工具图标")}/>:<Puzzle size={35}/>}</span><div><strong>{preview.manifest.name}</strong><p>{preview.manifest.description}</p></div></div><dl><dt>{uiText("标识")}</dt><dd>{preview.manifest.id}</dd><dt>{uiText("版本")}</dt><dd>{preview.manifest.version}</dd><dt>{uiText("类型")}</dt><dd>{preview.manifest.runtime==='exe'?uiText("Windows 应用"):uiText("本地 HTML 工具")}</dd><dt>{uiText("入口")}</dt><dd>{preview.manifest.entry}</dd></dl>{preview.existing_id&&<p className="local-tool-help">{uiText("已添加过这个工具，更新后保留原来的入口位置。")}</p>}</section>}
  <details className="local-tool-api"><summary>{uiText("工具开发接入格式")}</summary><p>{uiText("在工具文件夹中放置 UTF-8 编码的 scidesk-tool.json：")}</p><pre>{JSON.stringify(example,null,2)}</pre><p>{uiText("entry 和 icon 使用文件夹内的相对路径；icon、args 可省略。本地网页使用 runtime: \"html\" 和 entry: \"index.html\"。")}</p><p>{uiText("EXE 工具需自带运行所需文件。启动目录为工具文件夹；SCIDESK_TOOL_DATA_DIR 为独立数据目录，SCIDESK_TOOL_API_VERSION 为接口版本 1。")}</p></details>
 </div></Modal>
}
