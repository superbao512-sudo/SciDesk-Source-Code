import type {Idea} from '../../../public/ideas-mobile/core.js';
export function selectionBox(a:{x:number;y:number},b:{x:number;y:number}){return {x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(b.x-a.x),height:Math.abs(b.y-a.y)}}
export function selectedInBox(nodes:Idea[],box:{x:number;y:number;width:number;height:number}){
 if(box.width<2||box.height<2)return [];
 return nodes.filter(r=>!r.deleted_at&&r.canvas_x<box.x+box.width&&r.canvas_x+r.width>box.x&&r.canvas_y<box.y+box.height&&r.canvas_y+r.height>box.y).map(r=>r.id);
}
