import {useEffect,useRef} from 'react';
export interface MenuAction {label:string;run:()=>void;danger?:boolean;disabled?:boolean}
export function PopupMenu({x,y,actions,onClose}:{x:number;y:number;actions:MenuAction[];onClose:()=>void}){
 const ref=useRef<HTMLDivElement>(null);
 useEffect(()=>{const outside=(event:PointerEvent)=>{if(!ref.current?.contains(event.target as Node))onClose()};const key=(event:KeyboardEvent)=>{if(event.key==='Escape')onClose()};window.addEventListener('pointerdown',outside);window.addEventListener('keydown',key);ref.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();return()=>{window.removeEventListener('pointerdown',outside);window.removeEventListener('keydown',key)}},[onClose]);
 return <div ref={ref} className="project-popup-menu" role="menu" style={{left:Math.max(8,Math.min(x,window.innerWidth-224)),top:Math.max(8,Math.min(y,window.innerHeight-300))}}>{actions.map(item=><button role="menuitem" type="button" key={item.label} disabled={item.disabled} className={item.danger?'danger':''} onClick={()=>{item.run();onClose()}}>{item.label}</button>)}</div>;
}
