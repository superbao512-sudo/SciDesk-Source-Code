import type {ExtractionOptions,Pixel,Region} from './core';
export type Raster={width:number;height:number;data:Uint8ClampedArray|Uint8Array};
export type Extraction={points:Pixel[];uncertain:Region[];warnings:string[];matchedPixels:number};
export function extractCurve(image:Raster,roi:Region,options:ExtractionOptions):Extraction{
 if(!options.seed)throw new Error('请先用取色工具点击目标曲线，确定颜色和追踪起点');
 if(!/^#[0-9a-f]{6}$/i.test(options.color)||!Number.isFinite(options.tolerance)||options.tolerance<0||options.tolerance>250||!Number.isInteger(options.step)||options.step<1||options.step>100)throw new Error('颜色、容差或采样间隔无效');
 const x0=Math.max(0,Math.ceil(roi.x)),y0=Math.max(0,Math.ceil(roi.y)),x1=Math.min(image.width,Math.floor(roi.x+roi.w)),y1=Math.min(image.height,Math.floor(roi.y+roi.h)),width=x1-x0,height=y1-y0;
 if(width<2||height<2||width*height>20_000_000)throw new Error('ROI 无效或超过 2000 万像素，请缩小提取区域');
 const target=[1,3,5].map(offset=>parseInt(options.color.slice(offset,offset+2),16));
 if(target.every(c=>c>238))throw new Error('所选颜色接近白色背景，请重新取色');
 const mask=new Uint8Array(width*height),threshold=options.tolerance**2;let count=0;
 for(let y=0;y<height;y++)for(let x=0;x<width;x++){const at=((y+y0)*image.width+x+x0)*4;const distance=(image.data[at]-target[0])**2+(image.data[at+1]-target[1])**2+(image.data[at+2]-target[2])**2;if(image.data[at+3]>200&&distance<=threshold){mask[y*width+x]=1;count++;}}
 if(count>width*height*.35)throw new Error('匹配像素过多，可能选中了背景或网格；请缩小 ROI 或降低容差');
 const seedX=Math.round(options.seed.x)-x0,seedY=Math.round(options.seed.y)-y0;let seed=-1,best=Infinity;
 for(let y=Math.max(0,seedY-6);y<Math.min(height,seedY+7);y++)for(let x=Math.max(0,seedX-6);x<Math.min(width,seedX+7);x++){const d=(seedX-x)**2+(seedY-y)**2;if(mask[y*width+x]&&d<best){best=d;seed=y*width+x;}}
 if(seed<0)throw new Error('ROI 内的取色起点未命中目标颜色，请重新取色或调整容差');
 const queue=new Int32Array(count);let head=0,tail=1;queue[0]=seed;mask[seed]=2;
 while(head<tail){const index=queue[head++],x=index%width,y=Math.floor(index/width);for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){const nx=x+dx,ny=y+dy;if(nx<0||ny<0||nx>=width||ny>=height)continue;const next=ny*width+nx;if(mask[next]===1){mask[next]=2;queue[tail++]=next;}}}
 if(tail<4)throw new Error('起点附近的连通曲线像素不足');
 const points:Pixel[]=[],uncertain:Region[]=[],warnings:string[]=[];let previous=-1;
 for(let x=0;x<width;x++){
  const runs:{start:number;end:number}[]=[];
  for(let y=0;y<height;y++){if(mask[y*width+x]!==2)continue;const start=y;while(y+1<height&&mask[(y+1)*width+x]===2)y++;runs.push({start,end:y});}
  if(!runs.length)continue;
  if(runs.length>1||runs[0].end-runs[0].start>Math.max(12,height*.08)){
   const top=runs[0].start,bottom=runs.at(-1)!.end;const last=uncertain.at(-1);if(last&&last.x+last.w===x+x0){const bottomEdge=Math.max(last.y+last.h,bottom+y0+1);last.y=Math.min(last.y,top+y0);last.h=bottomEdge-last.y;last.w++;}else uncertain.push({x:x+x0,y:top+y0,w:1,h:bottom-top+1});continue;
  }
  if(previous<0||x-previous>=options.step){points.push({x:x+x0,y:(runs[0].start+runs[0].end)/2+y0});previous=x;}
 }
 if(uncertain.length)warnings.push('橙色区域存在分叉、同色重叠或过宽像素带，已跳过；请人工核对并补点。');
 // Locate disconnected same-color components as review regions, without bridging them.
 for(let start=0;start<mask.length;start++){
  if(mask[start]!==1)continue;let head=0,end=1;queue[0]=start;mask[start]=3;
  let minX=width,minY=height,maxX=0,maxY=0;
  while(head<end){const at=queue[head++],x=at%width,y=Math.floor(at/width);minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y);
   for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){const nx=x+dx,ny=y+dy;if(nx<0||ny<0||nx>=width||ny>=height)continue;const next=ny*width+nx;if(mask[next]===1){mask[next]=3;queue[end++]=next;}}
  }
  if(end>=4&&uncertain.length<64)uncertain.push({x:minX+x0,y:minY+y0,w:maxX-minX+1,h:maxY-minY+1});
 }
 if(tail<count)warnings.push(`ROI 中有 ${count-tail} 个同色像素未与起点连通（可能是断线、散点、图例或其他曲线），未跨越缺口连接。`);
 if(target.every(c=>c<60))warnings.push('目标颜色接近黑色，可能包含坐标轴、文字或网格，请仔细检查。');
 if(points.length<2)throw new Error('无法形成可确定的曲线采样，请使用手动拾点或缩小 ROI');
 warnings.push('自动结果尚未复核。请检查曲线、图例、网格和交叉点；不会默认平滑或填补缺口。');
 return {points,uncertain,warnings,matchedPixels:tail};
}
