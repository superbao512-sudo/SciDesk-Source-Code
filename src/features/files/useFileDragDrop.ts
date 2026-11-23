import {useEffect,useRef,useState,type PointerEvent,type RefObject} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {listen,type Event} from '@tauri-apps/api/event';
import type {DragDropEvent} from '@tauri-apps/api/webview';
import {selectFiles,physicalDropPoint} from './selection';
import {projectCommand} from '../projects/projectService';
import './fileDragDrop.css';
const listenDrop=(handler:(event:Event<DragDropEvent>)=>void)=>import('@tauri-apps/api/webviewWindow').then(({getCurrentWebviewWindow})=>getCurrentWebviewWindow().onDragDropEvent(handler));
export type FileTarget={id:string;label:string;relative:string;linkId?:string;association?:boolean};
export function useNativeCanvasFiles(ref:RefObject<HTMLDivElement|null>,enabled:boolean,onFiles:(files:File[],point:{x:number;y:number})=>Promise<void>,onError:(message:string)=>void){
 useEffect(()=>{
  if(!enabled||!isTauri())return;let active=true;let dispose:(()=>void)|undefined;
  void listenDrop(event=>{
   const p=event.payload;if(!active||p.type!=='drop')return;
   const point=physicalDropPoint(p.position.x,p.position.y,window.devicePixelRatio),element=document.elementFromPoint(point.x,point.y);
   if(!element||!ref.current?.contains(element))return;
   void (async()=>{for(const path of p.paths){if(!active)return;try{const data=await projectCommand<{name:string;mime:string;base64:string}>('read_drop_file',{path});if(!active)return;const bytes=Uint8Array.from(atob(data.base64),c=>c.charCodeAt(0));await onFiles([new File([bytes],data.name,{type:data.mime})],point)}catch(e){if(active)onError(String(e))}}})().catch(e=>{if(active)onError(String(e))});
  }).then(off=>{if(active)dispose=off;else off()}).catch(e=>{if(active)onError(String(e))});
  return()=>{active=false;dispose?.()};
 },[ref,enabled,onFiles,onError]);
}
export function useNativeFileDrop(ref:RefObject<HTMLDivElement|null>,enabled:boolean,onDrop:(paths:string[],target:FileTarget|null,mode:'copy'|'move')=>void,onError:(message:string)=>void){
 const [hover,setHover]=useState<string|null>(null);
 useEffect(()=>{
  if(!enabled||!isTauri())return;let active=true;const disposers:(()=>void)[]=[];
  const targetAt=(position:{x:number;y:number})=>{
   const point=physicalDropPoint(position.x,position.y,window.devicePixelRatio),element=document.elementFromPoint(point.x,point.y);
   if(!element||!ref.current?.contains(element))return undefined;
   const raw=element.closest<HTMLElement>('[data-file-drop-target]')?.dataset.fileDropTarget;
   try{return raw?JSON.parse(raw) as FileTarget:null}catch{return null}
  };
  const retain=(promise:Promise<()=>void>)=>{void promise.then(off=>{if(active)disposers.push(off);else off()}).catch(e=>{if(active)onError(`无法启用文件拖入：${String(e)}`)})};
  retain(listenDrop(event=>{
   if(!active)return;const p=event.payload;if(p.type==='leave'||p.type==='drop'){setHover(null);return}
   const target=targetAt(p.position);setHover(target===undefined?null:target?.id??'');
  }));
  retain(listen<{paths:string[];position:{x:number;y:number};mode:'copy'|'move'}>('scidesk://file-drop',event=>{
   if(!active)return;setHover(null);const target=targetAt(event.payload.position);
   if(target!==undefined)onDrop(event.payload.paths,target,event.payload.mode);
  }));
  return()=>{active=false;for(const off of disposers)off()};
 },[ref,enabled,onDrop,onError]);
 return hover;
}
export function useFileRows(order:string[],start:(keys:string[])=>Promise<void>,onError:(message:string)=>void){
 const [selected,setSelected]=useState<string[]>([]);const anchor=useRef<string|null>(null),gesture=useRef<{x:number;y:number;keys:string[]}|null>(null),suppress=useRef(0);
 useEffect(()=>{
  const move=(event:globalThis.PointerEvent)=>{
   const state=gesture.current;if(!state)return;if(!(event.buttons&1)){gesture.current=null;return}
   if(Math.hypot(event.clientX-state.x,event.clientY-state.y)<7)return;
   gesture.current=null;suppress.current=performance.now()+1000;
   if(!isTauri()){onError('请在 Windows 桌面版中拖出真实文件');return}
   void start(state.keys).catch(error=>onError(String(error))).finally(()=>{suppress.current=performance.now()+600});
  };
  const up=()=>{gesture.current=null};window.addEventListener('pointermove',move);window.addEventListener('pointerup',up);window.addEventListener('pointercancel',up);
  return()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',up);window.removeEventListener('pointercancel',up)};
 },[start,onError]);
 const pointer=(id:string,event:PointerEvent)=>{
  if(event.button!==0||event.target instanceof Element&&event.target.closest('input,select,textarea'))return;
  const next=!event.ctrlKey&&!event.metaKey&&!event.shiftKey&&selected.includes(id)?selected:selectFiles(order,selected,id,anchor.current,event.ctrlKey||event.metaKey,event.shiftKey);
  setSelected(next);if(!event.shiftKey)anchor.current=id;gesture.current={x:event.clientX,y:event.clientY,keys:next.includes(id)?next:[id]};
 };
 return {selected,pointer,setSelected,wasDragged:()=>performance.now()<suppress.current};
}
