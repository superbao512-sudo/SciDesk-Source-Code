import {uiText} from '../../../locales/language';
import {useEffect,useState} from 'react';
import {Modal} from '../../../components/common/Modal';
import {screenshotCommand,type CaptureSettings} from './service';
export function ScreenshotSettings({onClose}:{onClose:()=>void}){
 const [settings,setSettings]=useState<CaptureSettings>({enabled:true,shortcut:'Ctrl+Shift+F6',delay:0}),[error,setError]=useState(''),[notice,setNotice]=useState(''),[busy,setBusy]=useState(false);
 const load=()=>screenshotCommand<{settings:CaptureSettings;error:string|null}>('settings').then(value=>{setSettings(value.settings);setError(value.error??'')});
 useEffect(()=>{void load().catch(e=>setError(String(e)))},[]);
 const save=async()=>{setBusy(true);setError('');try{await screenshotCommand('configure',settings);await new Promise(resolve=>setTimeout(resolve,180));await load();setNotice(uiText("设置已保存"))}catch(e){setError(String(e))}finally{setBusy(false)}};
 return <Modal title={uiText("高级截图设置")} onClose={onClose} closeDisabled={busy} footer={<><button className="button" disabled={busy} onClick={onClose}>{uiText("关闭")}</button><button className="button primary" disabled={busy} onClick={()=>void save()}>{uiText("保存设置")}</button></>}><div className="tool-form"><label><span><input type="checkbox" checked={settings.enabled} onChange={e=>setSettings({...settings,enabled:e.target.checked})}/>{uiText("启用全局截图快捷键")}</span></label><label>{uiText("快捷键")}<input value={settings.shortcut} onChange={e=>setSettings({...settings,shortcut:e.target.value})} placeholder="Ctrl+Shift+F6"/></label><small>{uiText("使用 Ctrl / Alt / Shift 与字母、数字或 F1–F12。系统保留的 Win 组合键不可用。")}</small><label>{uiText("延时截图")}<select value={settings.delay} onChange={e=>setSettings({...settings,delay:Number(e.target.value) as 0|3|5})}><option value={0}>{uiText("立即截图")}</option><option value={3}>{uiText("3 秒")}</option><option value={5}>{uiText("5 秒")}</option></select></label>{error&&<p className="projects-error" role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}</div></Modal>;
}
