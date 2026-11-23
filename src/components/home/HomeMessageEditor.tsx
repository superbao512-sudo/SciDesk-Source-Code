import {uiText} from '../../locales/language';
import {useState} from 'react';
import {Modal} from '../common/Modal';
import {useApplicationAppearance} from '../appearance/appearanceContext';
import {setSetting} from '../../features/settings/settingsService';
export function HomeMessageEditor({field,onClose}:{field:'subtitle'|'message';onClose:()=>void}){
 const {config}=useApplicationAppearance();const subtitle=field==='subtitle',key=subtitle?'home.subtitle_text':'home.message',label=subtitle?'首页副标题':'首页个性文字';const [text,setText]=useState(config[key]),[busy,setBusy]=useState(false),[error,setError]=useState('');
 async function save(){if(busy)return;setBusy(true);try{await setSetting(key,subtitle?text.trim():text.trim()||'专注于重要的科学问题，一步一个脚印。');onClose()}catch(reason){setError(String(reason))}finally{setBusy(false)}}
 return <Modal title={uiText("编辑")+label} closeDisabled={busy} onClose={onClose} footer={<div className="modal-actions"><button className="button" disabled={busy} onClick={onClose}>{uiText("取消")}</button><button className="button primary" disabled={busy} onClick={()=>void save()}>{busy?uiText("正在保存…"):uiText("保存")}</button></div>}><div className="form"><label>{label}{subtitle?<input autoFocus aria-label={label} maxLength={40} value={text} onChange={event=>setText(event.target.value)}/>:<textarea autoFocus aria-label={uiText("首页个性文字")} maxLength={120} rows={3} value={text} onChange={event=>setText(event.target.value)}/>}</label>{error&&<p role="alert" className="form-error">{error}</p>}</div></Modal>;
}
