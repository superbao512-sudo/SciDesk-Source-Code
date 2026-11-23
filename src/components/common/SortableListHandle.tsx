import {uiText} from '../../locales/language';
import {useRef} from 'react';
import {GripVertical} from 'lucide-react';

// Only the handle initiates dragging; cards retain their click actions.
export function SortableListHandle({id,name,idAttribute,onTarget,onReorder,onMove}:{id:string;name:string;idAttribute:'data-project-id'|'data-manual-id'|'data-chapter-id';onTarget:(id:string)=>void;onReorder:(source:string,target:string)=>void;onMove:(delta:number)=>void}){
 const dragging=useRef(false);
 const targetAt=(x:number,y:number)=>document.elementFromPoint(x,y)?.closest<HTMLElement>(`[${idAttribute}]`)?.getAttribute(idAttribute)||'';
 return <button className="project-drag-handle" title={uiText("拖动排序（也可按上下方向键）")} aria-label={uiText("拖动排序 {0}",name)} onClick={e=>e.stopPropagation()} onKeyDown={e=>{if(e.key==='ArrowUp'||e.key==='ArrowDown'){e.preventDefault();e.stopPropagation();onMove(e.key==='ArrowUp'?-1:1)}}} onPointerDown={e=>{if(e.button!==0)return;e.stopPropagation();dragging.current=true;e.currentTarget.setPointerCapture(e.pointerId)}} onPointerMove={e=>{if(!dragging.current)return;onTarget(targetAt(e.clientX,e.clientY));const list=e.currentTarget.closest('.project-list-rows');if(list){const r=list.getBoundingClientRect();if(e.clientY<r.top+28)list.scrollTop-=12;else if(e.clientY>r.bottom-28)list.scrollTop+=12}}} onPointerUp={e=>{if(!dragging.current)return;dragging.current=false;e.currentTarget.releasePointerCapture(e.pointerId);const target=targetAt(e.clientX,e.clientY);onTarget('');if(target)onReorder(id,target)}} onPointerCancel={()=>{dragging.current=false;onTarget('')}}><GripVertical size={16}/></button>
}
