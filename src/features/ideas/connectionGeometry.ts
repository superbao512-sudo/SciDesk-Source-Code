import type {Idea} from '../../../public/ideas-mobile/core.js';
export type Anchor='top'|'right'|'bottom'|'left';
export const anchors:Anchor[]=['top','right','bottom','left'];
export function anchorPoint(node:Idea,side:Anchor){
 const x=node.canvas_x,y=node.canvas_y,w=node.width,h=node.height;
 return {top:{x:x+w/2,y},right:{x:x+w,y:y+h/2},bottom:{x:x+w/2,y:y+h},left:{x,y:y+h/2}}[side];
}
export function screenToWorld(point:{x:number;y:number},origin:{left:number;top:number},view:{x:number;y:number;zoom:number}){return {x:(point.x-origin.left-view.x)/view.zoom,y:(point.y-origin.top-view.y)/view.zoom};}
export function connectionTarget(nodes:Idea[],x:number,y:number,exclude:string){
 for(const node of [...nodes].map((n,i)=>({n,i})).sort((a,b)=>(b.n.z_index||0)-(a.n.z_index||0)||b.i-a.i).map(v=>v.n)){
  if(node.id===exclude||node._entity==='room'||node.deleted_at)continue;
  const inside=x>=node.canvas_x&&x<=node.canvas_x+node.width&&y>=node.canvas_y&&y<=node.canvas_y+node.height;if(!inside&&!anchors.some(s=>{const p=anchorPoint(node,s);return Math.hypot(x-p.x,y-p.y)<=8}))continue;
  const side=anchors.reduce((best,next)=>{const a=anchorPoint(node,best),b=anchorPoint(node,next);return Math.hypot(x-b.x,y-b.y)<Math.hypot(x-a.x,y-a.y)?next:best},'right');
  return {id:node.id,anchor:side};
 }return null;
}
