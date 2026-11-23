import {uiText} from '../../../locales/language';
import {useEffect,useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
export function ScreenshotEvents(){
 const navigate=useNavigate(),[notice,setNotice]=useState<{message:string;error:boolean}|null>(null);
 useEffect(()=>{
  if(!isTauri())return;let timer:ReturnType<typeof setTimeout>|undefined;
  const show=(message:string,error:boolean)=>{setNotice({message,error});clearTimeout(timer);timer=setTimeout(()=>setNotice(null),8000)};
  const registrations=[listen('screenshot:digitize',()=>navigate('/tools/plot-digitizer',{state:{requestId:crypto.randomUUID()}})),listen<string>('screenshot:error',event=>show(event.payload,true)),listen<string>('screenshot:result',event=>show(event.payload,false))];
  return()=>{clearTimeout(timer);void Promise.all(registrations).then(rows=>rows.forEach(unlisten=>unlisten()))};
 },[navigate]);
 return notice?<div role={notice.error?'alert':'status'} style={{position:'fixed',zIndex:9999,right:18,bottom:18,maxWidth:480,padding:'12px 16px',background:notice.error?'#fff1f0':'#eef8f2',color:notice.error?'#a34040':'#287452',border:'1px solid #c9d8d1',borderRadius:6,fontSize:12,boxShadow:'0 4px 18px #24385220'}}>{notice.message}<button onClick={()=>setNotice(null)} aria-label={uiText("关闭截图提示")} style={{marginLeft:12,border:0,background:'none',color:'inherit',cursor:'pointer'}}>×</button></div>:null;
}
