import {uiText} from '../../locales/language';
import {useEffect,useMemo,useRef,useState} from 'react';
import {type Idea,type Edge,type CanvasState} from '../../../public/ideas-mobile/core.js';
import {type Anchor} from './connectionGeometry';
import {type Commit} from './IdeasRooms';
import {message} from './ideasService';
import {routeConnections,roundedPath,previewRoute} from './connectionRouting';
const styles=[['solid','实线'],['dashed','虚线'],['dashdot','点划线'],['dotted','点线']] as const;
const dash={solid:undefined,dashed:'9 6',dashdot:'12 5 2 5',dotted:'1 6'};
export function Connections({edges,nodes,selected,onSelect,onContext,wire,source,view}:{edges:Edge[];nodes:Idea[];selected:string;onSelect:(id:string)=>void;onContext:(id:string,x:number,y:number)=>void;wire:{x:number;y:number;target:string;sourceAnchor:Anchor;targetAnchor:Anchor}|null;source?:Idea;view:CanvasState}){
 const routingKey=JSON.stringify({nodes:nodes.map(n=>({id:n.id,canvas_x:n.canvas_x,canvas_y:n.canvas_y,width:n.width,height:n.height})),edges:edges.map(e=>({id:e.id,source_idea_id:e.source_idea_id,target_idea_id:e.target_idea_id,source_anchor:e.source_anchor,target_anchor:e.target_anchor}))});
 const routes=useMemo(()=>{const data=JSON.parse(routingKey);return new Map(routeConnections(data.nodes,data.edges).map(r=>[r.id,r]))},[routingKey]);
 return <svg className="idea-connections" aria-label={uiText("想法连接网络")} width="100%" height="100%"><g transform={'translate('+view.x+' '+view.y+') scale('+view.zoom+')'}>{edges.map(e=>{
  const a=nodes.find(r=>r.id===e.source_idea_id),b=nodes.find(r=>r.id===e.target_idea_id);if(!a||!b)return null;
  const route=routes.get(e.id);if(!route)return null;const p=route.start,q=route.end,d=roundedPath(route.points,nodes,view.zoom);
  return <g key={e.id} data-edge-id={e.id} data-source-id={e.source_idea_id} data-target-id={e.target_idea_id} className={selected===e.id?'selected':''}><title>{route.hits?uiText("节点重叠或通道受限，请移动卡片以改善路径"):uiText("自动路由连接")}</title>
   <path className="idea-edge-hit" vectorEffect="non-scaling-stroke" d={d} role="button" tabIndex={0} aria-label={uiText("连接 ")+(e.relation_type||uiText("普通关联"))+' '+(e.label||'')} aria-pressed={selected===e.id} onPointerDown={event=>event.stopPropagation()} onClick={event=>{event.stopPropagation();onSelect(e.id)}} onContextMenu={event=>{event.preventDefault();event.stopPropagation();onContext(e.id,event.clientX,event.clientY)}} onKeyDown={event=>{if(event.key==='Enter')onSelect(e.id);if(event.key==='ContextMenu'||event.shiftKey&&event.key==='F10'){event.preventDefault();const box=event.currentTarget.getBoundingClientRect();onContext(e.id,box.x+box.width/2,box.y+box.height/2)}}}/>
   <path className="idea-edge-line" d={d} style={{stroke:e.color||'#8199ba',strokeDasharray:dash[e.line_style||'solid']}}/>
   {(e.relation_type||e.label)&&<text x={(p.x+q.x)/2} y={(p.y+q.y)/2-8} textAnchor="middle">{e.relation_type||e.label.slice(0,20)}</text>}
  </g>;
 })}{wire&&source&&<path className="idea-edge-draft" d={roundedPath(previewRoute(source,wire,wire.sourceAnchor,nodes.find(r=>r.id===wire.target),wire.targetAnchor,nodes),nodes,view.zoom)}/>}</g></svg>;
}
export function LineMenu({edge,x,y,onCommit,onClose}:{edge:Edge;x:number;y:number;onCommit:Commit;onClose:()=>void}){
 const menu=useRef<HTMLDivElement>(null),[error,setError]=useState('');
 useEffect(()=>{const outside=(event:PointerEvent)=>{if(!menu.current?.contains(event.target as Node))onClose()},key=(event:KeyboardEvent)=>{if(event.key==='Escape')onClose()};window.addEventListener('pointerdown',outside);window.addEventListener('keydown',key);return()=>{window.removeEventListener('pointerdown',outside);window.removeEventListener('keydown',key)}},[onClose]);
 const save=async(patch:Record<string,unknown>)=>{try{await onCommit(edge.id,patch,edge.version,undefined,'edge');setError('')}catch(e){setError(message(e))}};
 return <div ref={menu} className="idea-line-menu" role="menu" aria-label={uiText("连接线样式")} style={{left:Math.max(8,Math.min(x,window.innerWidth-238)),top:Math.max(8,Math.min(y,window.innerHeight-310))}} onContextMenu={e=>e.preventDefault()}>
  <small>{uiText("线条样式")}</small>{styles.map(([value,label])=><button key={value} role="menuitemradio" aria-checked={(edge.line_style||'solid')===value} onClick={()=>void save({line_style:value})}><svg width="40" height="12" aria-hidden="true"><line x1="0" y1="6" x2="40" y2="6" stroke="currentColor" strokeWidth="2" strokeDasharray={dash[value]} strokeLinecap="round"/></svg>{label}</button>)}
  <label>{uiText("线条颜色")}<input aria-label={uiText("线条颜色")} type="color" value={edge.color||'#8199ba'} onChange={e=>void save({color:e.target.value})}/></label>
  <div className="idea-line-colors">{['#8199ba','#2563eb','#16a085','#d97706','#dc2626','#9333ea'].map(color=><button key={color} aria-label={uiText("颜色 ")+color} style={{background:color}} onClick={()=>void save({color})}/>)}</div>
  <button role="menuitem" className="danger" onClick={()=>{void save({deleted:true});onClose()}}>{uiText("删除连接")}</button>{error&&<small role="alert">{error}</small>}
 </div>;
}
