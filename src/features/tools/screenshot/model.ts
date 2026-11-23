export type Point={x:number;y:number};
export type Box={x:number;y:number;w:number;h:number};
export type MarkKind='arrow'|'rectangle'|'ellipse'|'pen'|'text'|'number'|'highlight'|'mosaic'|'blur';
export type Mark=Box&{id:string;kind:MarkKind;color:string;lineWidth:number;points:Point[];text:string;number:number};
export type History={past:Mark[][];present:Mark[];future:Mark[][]};
export const emptyHistory=():History=>({past:[],present:[],future:[]});
export function commit(history:History,marks:Mark[]):History{return {past:[...history.past,history.present].slice(-80),present:marks,future:[]};}
export function undo(history:History):History{return history.past.length?{past:history.past.slice(0,-1),present:history.past.at(-1)!,future:[history.present,...history.future]}:history;}
export function redo(history:History):History{return history.future.length?{past:[...history.past,history.present],present:history.future[0],future:history.future.slice(1)}:history;}
export function boxFrom(a:Point,b:Point):Box{return{x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),w:Math.max(1,Math.abs(a.x-b.x)),h:Math.max(1,Math.abs(a.y-b.y))};}
export function imagePoint(client:Point,viewport:Box,pixels:{width:number;height:number}):Point{return{x:Math.max(0,Math.min(pixels.width,(client.x-viewport.x)*pixels.width/viewport.w)),y:Math.max(0,Math.min(pixels.height,(client.y-viewport.y)*pixels.height/viewport.h))};}
export function clampBox(box:Box,bounds:Box):Box{const x=Math.max(bounds.x,Math.min(bounds.x+bounds.w-1,box.x)),y=Math.max(bounds.y,Math.min(bounds.y+bounds.h-1,box.y));return{x,y,w:Math.max(1,Math.min(box.w,bounds.x+bounds.w-x)),h:Math.max(1,Math.min(box.h,bounds.y+bounds.h-y))};}
export function inside(p:Point,b:Box,padding=0){return p.x>=b.x-padding&&p.y>=b.y-padding&&p.x<=b.x+b.w+padding&&p.y<=b.y+b.h+padding;}
export function makeMark(kind:MarkKind,a:Point,b:Point,color:string,lineWidth:number,number=1):Mark{return {...boxFrom(a,b),id:crypto.randomUUID(),kind,color,lineWidth,points:[a,b],text:kind==='text'?'':String(number),number};}
export function transformMark(mark:Mark,next:Box):Mark{
 return {...mark,...next,points:mark.points.map(p=>({x:next.x+(p.x-mark.x)*next.w/mark.w,y:next.y+(p.y-mark.y)*next.h/mark.h}))};
}
function distance(p:Point,a:Point,b:Point){const dx=b.x-a.x,dy=b.y-a.y;const t=Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/(dx*dx+dy*dy||1)));return Math.hypot(p.x-a.x-t*dx,p.y-a.y-t*dy);}
export function hitMark(marks:Mark[],p:Point,tolerance=8):Mark|undefined{
 return [...marks].reverse().find(mark=>{
  if(!inside(p,mark,tolerance))return false;
  if(mark.kind==='pen'||mark.kind==='arrow')return mark.points.slice(1).some((point,i)=>distance(p,mark.points[i],point)<=Math.max(tolerance,mark.lineWidth));
  return true;
 });
}
export function relativeRect(rect:{x:number;y:number;width:number;height:number},origin:{x:number;y:number}):Box{return{x:rect.x-origin.x,y:rect.y-origin.y,w:rect.width,h:rect.height};}
