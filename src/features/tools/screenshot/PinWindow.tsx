import {uiText} from '../../../locales/language';
import {useEffect,useState} from 'react';
import {screenshotCommand} from './service';
import './screenshot.css';
export function PinWindow(){
 const [image,setImage]=useState(''),[error,setError]=useState(''),[scale,setScale]=useState(1),[menu,setMenu]=useState(false);
 const call=(op:string,args:object={})=>{void screenshotCommand(op,args).catch(e=>setError(String(e)))};
 useEffect(()=>{let active=true;void screenshotCommand<{image:string}>('pin_frame').then(value=>{if(active)setImage(value.image)}).catch(e=>setError(String(e)));return()=>{active=false}},[]);
 useEffect(()=>{const key=(e:KeyboardEvent)=>{if(e.key==='Escape'){void screenshotCommand('pin_close');}if(['+','=','-','0'].includes(e.key)){e.preventDefault();const next=e.key==='0'?1:Math.max(0.1,Math.min(4,scale*(e.key==='-'?1/1.1:1.1)));void screenshotCommand('pin_zoom',{scale:next}).then(()=>setScale(next)).catch(e=>setError(String(e)))}};window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key)},[scale]);
 return <main className="pin-root" aria-label={uiText("截图贴图")} onContextMenu={e=>{e.preventDefault();setMenu(!menu)}} onWheel={e=>{const next=Math.max(0.1,Math.min(4,scale*(e.deltaY<0?1.1:1/1.1)));void screenshotCommand('pin_zoom',{scale:next}).then(()=>setScale(next)).catch(e=>setError(String(e)))}}>
  {image&&<img src={image} alt={uiText("截图贴图 {0}%，拖动移动，滚轮缩放，Esc 关闭",Math.round(scale*100))} draggable={false} onLoad={()=>{void screenshotCommand<{scale:number}>('pin_ready').then(value=>setScale(value.scale)).catch(e=>setError(String(e)))}} onError={()=>call('pin_close')} onPointerDown={e=>{if(e.button===0){setMenu(false);call('pin_drag')}}}/>}
  <button className="pin-close" aria-label={uiText("关闭贴图")} title={uiText("关闭贴图 (Esc)")} onClick={()=>call('pin_close')}>×</button><span className="pin-resize" title={uiText("调整贴图大小")} onPointerDown={()=>call('pin_resize')}/>
  {menu&&<div className="pin-menu"><button onClick={()=>{setScale(1);call('pin_zoom',{scale:1});setMenu(false)}}>{uiText("原始尺寸")}</button><button onClick={()=>call('pin_close')}>{uiText("关闭贴图")}</button></div>}{error&&<p role="alert">{error}</p>}
 </main>;
}
