import {uiText} from '../../locales/language';
import {useState} from 'react';
import {Modal} from '../../components/common/Modal';
import {journalCommand,errorMessage} from './journalService';
import {readSavedScholar} from './savedScholar';
import type {ScholarProfile} from './types';

export function ScholarImport({profile,onClose,onSave}:{profile:ScholarProfile;onClose:()=>void;onSave:(p:ScholarProfile)=>void}){
 const [preview,setPreview]=useState<ReturnType<typeof readSavedScholar>|null>(null);
 const [busy,setBusy]=useState(false),[error,setError]=useState('');
 async function choose(file?:File){
  if(!file)return;
  setBusy(true);setPreview(null);setError('');
  try{
   if(!/\.html?$/i.test(file.name)||file.size>16*1024*1024)throw Error('请选择 16 MB 以内的 .html 或 .htm 文件');
   const data=readSavedScholar(await file.text());
   if(profile.url&&new URL(profile.url).searchParams.get('user')!==new URL(data.url).searchParams.get('user'))throw Error('文件属于另一位研究人员，请选择当前主页的 HTML。');
   setPreview(data);
  }catch(e){setError(errorMessage(e))}finally{setBusy(false)}
 }
 async function save(){
  if(!preview)return;setBusy(true);setError('');
  try{onSave(await journalCommand<ScholarProfile>('profile_import_html',preview))}catch(e){setError(errorMessage(e));setBusy(false)}
 }
 return <Modal title={uiText("导入 Scholar 主页")} closeDisabled={busy} onClose={onClose} footer={<div className="modal-actions"><button className="button" disabled={busy} onClick={onClose}>{uiText("取消")}</button><button className="button primary" disabled={busy||!preview} onClick={()=>void save()}>{uiText("确认导入")}</button></div>}>
 <p>{uiText("在浏览器打开个人主页，点击论文列表底部的「显示更多」直到所需论文全部显示，再按 Ctrl+S 保存为 HTML。选择保存的 .html 文件即可。")}</p>
 <label>{uiText("主页 HTML 文件")}<input type="file" accept=".htm,.html" disabled={busy} onChange={e=>void choose(e.target.files?.[0])}/></label>
 {preview&&<div><p><strong>{preview.snapshot.name}</strong> · {preview.snapshot.affiliation}</p><p>{preview.snapshot.interests.length}{uiText("个研究方向，")}{preview.snapshot.works.length}{uiText("篇论文")}</p>
 {profile.snapshot&&<p>{uiText("确认后将替换当前主页缓存（原有")}{profile.snapshot.works.length}{uiText("篇论文）。")}</p>}</div>}
 {error&&<p role="alert">{error}</p>}</Modal>;
}
