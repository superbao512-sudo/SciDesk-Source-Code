import {useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {RefreshCw} from 'lucide-react';
import {Modal} from '../components/common/Modal';
import {version} from '../../package.json';

export function CheckUpdates({collapsed}:{collapsed:boolean}){
 const [visible,setVisible]=useState(false),[busy,setBusy]=useState(false),[message,setMessage]=useState('');
 async function check(){
  setVisible(true);setBusy(true);setMessage('正在查询 GitHub 正式发布版本…');
  try{
   const release=await invoke<[string,boolean]|null>('check_release');
   setMessage(!release?'仓库暂未发布正式版本。':release[1]?`发现新版本 ${release[0]}。请前往发布页下载安装包。`:`当前版本已是最新或高于已发布版本（${release[0]}）。`);
  }catch(error){setMessage(String(error))}finally{setBusy(false)}
 }
 return <><button type="button" className="button" style={{alignSelf:'center'}} title="检查更新" aria-label="检查更新" disabled={busy} onClick={()=>void check()}><RefreshCw size={15}/>{!collapsed&&'检查更新'}</button>
 {visible&&<Modal title="检查更新" onClose={()=>setVisible(false)}><p>当前版本 {version}</p><p role="status">{message}</p><button className="button" onClick={()=>void invoke('open_release_page').catch(e=>setMessage(String(e)))}>打开 GitHub 发布页</button></Modal>}</>;
}
