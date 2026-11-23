import type {Anchor} from './connectionGeometry';
export interface Bounds {id:string;canvas_x:number;canvas_y:number;width:number;height:number}
export interface Relation {id:string;source_idea_id:string;target_idea_id:string;source_anchor?:Anchor;target_anchor?:Anchor}
export interface Point {x:number;y:number}
type Rect={left:number;right:number;top:number;bottom:number};
const sides:Anchor[]=['right','bottom','left','top'];
const normal={right:{x:1,y:0},bottom:{x:0,y:1},left:{x:-1,y:0},top:{x:0,y:-1}};
const rect=(n:Bounds,pad=0):Rect=>({left:n.canvas_x-pad,right:n.canvas_x+n.width+pad,top:n.canvas_y-pad,bottom:n.canvas_y+n.height+pad});
export function port(n:Bounds,side:Anchor,fraction=.5):Point {const r=rect(n);return {right:{x:r.right,y:r.top+n.height*fraction},left:{x:r.left,y:r.top+n.height*fraction},top:{x:r.left+n.width*fraction,y:r.top},bottom:{x:r.left+n.width*fraction,y:r.bottom}}[side]}
export function crosses(a:Point,b:Point,r:Rect){return a.x===b.x?a.x>r.left&&a.x<r.right&&Math.max(a.y,b.y)>r.top&&Math.min(a.y,b.y)<r.bottom:a.y>r.top&&a.y<r.bottom&&Math.max(a.x,b.x)>r.left&&Math.min(a.x,b.x)<r.right}
function simplify(points:Point[]){const out:Point[]=[];for(const p of points){if(out.at(-1)?.x===p.x&&out.at(-1)?.y===p.y)continue;while(out.length>1){const a=out[out.length-2],b=out[out.length-1];if((a.x===b.x&&b.x===p.x)&&(b.y-a.y)*(p.y-b.y)>=0||(a.y===b.y&&b.y===p.y)&&(b.x-a.x)*(p.x-b.x)>=0)out.pop();else break;}out.push(p);}return out;}
function score(points:Point[],obstacles:Rect[]){let length=0,hits=0;for(let i=1;i<points.length;i++){length+=Math.abs(points[i].x-points[i-1].x)+Math.abs(points[i].y-points[i-1].y);hits+=obstacles.filter(r=>crosses(points[i-1],points[i],r)).length;}return {cost:length+Math.max(0,points.length-2)*18+hits*100000,hits};}
// Candidate routing stays bounded. Visibility-grid fallback is used only when all
// short candidates cross obstacles; edges join adjacent clear grid intersections.
function gridRoute(a:Point,b:Point,obstacles:Rect[]):Point[]|null {
 const nearby=obstacles.filter(r=>r.right>=Math.min(a.x,b.x)-250&&r.left<=Math.max(a.x,b.x)+250&&r.bottom>=Math.min(a.y,b.y)-250&&r.top<=Math.max(a.y,b.y)+250).slice(0,70);
 const xs=[...new Set([a.x,b.x,...nearby.flatMap(r=>[r.left,r.right])])].sort((a,b)=>a-b),ys=[...new Set([a.y,b.y,...nearby.flatMap(r=>[r.top,r.bottom])])].sort((a,b)=>a-b);
 const w=xs.length,start=ys.indexOf(a.y)*w+xs.indexOf(a.x),end=ys.indexOf(b.y)*w+xs.indexOf(b.x),point=(i:number)=>({x:xs[i%w],y:ys[Math.floor(i/w)]});
 const dist=new Map<number,number>(),previous=new Map<number,number>(),open:{key:number;cost:number;estimate:number}[]=[];
 const initial=start*3;dist.set(initial,0);open.push({key:initial,cost:0,estimate:0});let expanded=0;
 while(open.length&&expanded++<22000){open.sort((a,b)=>b.estimate-a.estimate);const current=open.pop()!;if(current.cost!==dist.get(current.key))continue;const index=Math.floor(current.key/3),direction=current.key%3,p=point(index);
  if(index===end){const path:Point[]=[];let key:number|undefined=current.key;while(key!==undefined){path.push(point(Math.floor(key/3)));key=previous.get(key);}return simplify(path.reverse());}
  const ix=index%w,iy=Math.floor(index/w),neighbors=[ix>0?index-1:-1,ix<w-1?index+1:-1,iy>0?index-w:-1,iy<ys.length-1?index+w:-1];
  for(const next of neighbors){if(next<0)continue;const q=point(next);if(obstacles.some(r=>crosses(p,q,r)))continue;const d=p.x===q.x?2:1,key=next*3+d,cost=current.cost+Math.abs(q.x-p.x)+Math.abs(q.y-p.y)+(direction&&direction!==d?18:0);if(cost>=(dist.get(key)??Infinity))continue;dist.set(key,cost);previous.set(key,current.key);open.push({key,cost,estimate:cost+Math.abs(q.x-b.x)+Math.abs(q.y-b.y)});}
 }return null;
}
function route(a:Bounds,b:Bounds,sa:Anchor,ta:Anchor,nodes:Bounds[],af=.5,bf=.5,fallback=false){
 const start=port(a,sa,af),end=port(b,ta,bf),n=normal[sa],m=normal[ta],out={x:start.x+n.x*16,y:start.y+n.y*16},entry={x:end.x+m.x*16,y:end.y+m.y*16};
 const obstacles=nodes.map(node=>rect(node,node.id===a.id||node.id===b.id?0:10));
 const candidates:Point[][]=[[out,{x:entry.x,y:out.y},entry],[out,{x:out.x,y:entry.y},entry]];
 const detours=nodes.filter(r=>r.canvas_x+r.width>=Math.min(start.x,end.x)-120&&r.canvas_x<=Math.max(start.x,end.x)+120&&r.canvas_y+r.height>=Math.min(start.y,end.y)-120&&r.canvas_y<=Math.max(start.y,end.y)+120).slice(0,70);
 for(const x of [...new Set([(out.x+entry.x)/2,...detours.flatMap(r=>[r.canvas_x-16,r.canvas_x+r.width+16])])])candidates.push([out,{x,y:out.y},{x,y:entry.y},entry]);
 for(const y of [...new Set([(out.y+entry.y)/2,...detours.flatMap(r=>[r.canvas_y-16,r.canvas_y+r.height+16])])])candidates.push([out,{x:out.x,y},{x:entry.x,y},entry]);
 let best={points:[] as Point[],cost:Infinity,hits:Infinity};for(const candidate of candidates){const points=simplify([start,...candidate,end]),s=score(points,obstacles);if(s.cost<best.cost)best={points,...s};}
 if(best.hits&&fallback){const middle=gridRoute(out,entry,obstacles);if(middle){const points=simplify([start,...middle,end]),s=score(points,obstacles);if(s.cost<best.cost)best={points,...s};}}
 return {...best,start,end,sourceSide:sa,targetSide:ta};
}
export function roundedPath(points:Point[],obstacles:Bounds[]=[],zoom=1){
 if(!points.length)return '';let d=`M ${points[0].x} ${points[0].y}`;
 for(let i=1;i<points.length-1;i++){const a=points[i-1],b=points[i],c=points[i+1],length1=Math.hypot(b.x-a.x,b.y-a.y),length2=Math.hypot(c.x-b.x,c.y-b.y);let radius=Math.min(7/Math.max(.5,zoom),length1/2,length2/2);
  // Never round toward the interior of a nearby card.
  if(obstacles.some(n=>{const r=rect(n);return b.x+radius>r.left&&b.x-radius<r.right&&b.y+radius>r.top&&b.y-radius<r.bottom}))radius=0;
  if(!radius){d+=` L ${b.x} ${b.y}`;continue;}const enter={x:b.x+(a.x-b.x)*radius/length1,y:b.y+(a.y-b.y)*radius/length1},exit={x:b.x+(c.x-b.x)*radius/length2,y:b.y+(c.y-b.y)*radius/length2};d+=` L ${enter.x} ${enter.y} Q ${b.x} ${b.y} ${exit.x} ${exit.y}`;
 }const end=points.at(-1)!;return d+` L ${end.x} ${end.y}`;
}
export function routeConnections(nodes:Bounds[],edges:Relation[]){
 const lookup=new Map(nodes.map(n=>[n.id,n])),plans=edges.flatMap(edge=>{const a=lookup.get(edge.source_idea_id),b=lookup.get(edge.target_idea_id);if(!a||!b||a.id===b.id)return [];
  let best:ReturnType<typeof route>|null=null,rank=Infinity;
  for(const sa of sides)for(const ta of sides){const candidate=route(a,b,sa,ta,nodes),value=candidate.cost+(sa!==edge.source_anchor?8:0)+(ta!==edge.target_anchor?8:0);if(value<rank){rank=value;best=candidate;}}
  return [{edge,a,b,plan:best!,af:.5,bf:.5}];});
 const groups=new Map<string,{entry:typeof plans[number];end:'af'|'bf';along:number}[]>();
 for(const entry of plans)for(const end of ['af','bf'] as const){const node=end==='af'?entry.a:entry.b,other=end==='af'?entry.b:entry.a,side=end==='af'?entry.plan.sourceSide:entry.plan.targetSide,key=node.id+':'+side;const list=groups.get(key)||[];list.push({entry,end,along:side==='top'||side==='bottom'?other.canvas_x+other.width/2:other.canvas_y+other.height/2});groups.set(key,list);}
 for(const group of groups.values()){group.sort((a,b)=>a.along-b.along||a.entry.edge.id.localeCompare(b.entry.edge.id));group.forEach((p,i)=>{p.entry[p.end]=group.length===1?.5:.2+.6*(i+1)/(group.length+1)});}
 return plans.map(p=>({id:p.edge.id,...route(p.a,p.b,p.plan.sourceSide,p.plan.targetSide,nodes,p.af,p.bf,true)}));
}
export function previewRoute(source:Bounds,pointer:Point,sourceSide:Anchor,target:Bounds|undefined,targetSide:Anchor,nodes:Bounds[]){
 const ghost=target||{id:'__pointer__',canvas_x:pointer.x,canvas_y:pointer.y,width:0,height:0};
 return route(source,ghost,sourceSide,targetSide,target?nodes:[...nodes,ghost],.5,.5,true).points;
}
