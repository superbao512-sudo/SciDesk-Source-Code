export type Table={name:string;headers:string[];rows:(number|null)[][]};
const numeric=(s:string)=>/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[ed][+-]?\d+)?$/i.test(s);
function fields(line:string,sep:string){
 if(!sep)return line.trim().split(/\s+/);
 const out:string[]=[];let value='',quoted=false;
 for(let i=0;i<line.length;i++){
  const c=line[i];
  if(c==='"'){if(quoted&&line[i+1]==='"'){value+='"';i++}else quoted=!quoted}
  else if(!quoted&&c===sep){out.push(value.trim());value=''}else value+=c;
 }
 if(quoted)throw Error('引号未闭合，请检查数据文件');
 out.push(value.trim());return out;
}
export function parseTable(name:string,text:string):Table{
 if(text.length>8*1024*1024)throw Error('单个数据文件最多 8 MB');
 const lines=text.replace(/^\uFEFF/,'').split(/\r\n?|\n/);
 let sep:string|undefined,headers:string[]=[],width=0,started=false,comment='',gap=false,count=0;
 const rows:(number|null)[][]=[];
 for(const [index,line] of lines.entries()){
  const s=line.trim();if(!s){if(started)gap=true;continue}
  if(/^[#!]/.test(s)){if(!started)comment=s.slice(1).trim();continue}
  sep??=s.includes('\t')?'\t':s.includes(',')?',':s.includes(';')?';':'';
  const cols=fields(s,sep);if(!width){width=cols.length;if(width<2||width>256)throw Error('数据应包含 2–256 列');
   if(!cols.some(numeric)&&!cols.every(v=>/^(?:nan|na|null|-|)$/i.test(v))){headers=cols;continue}
   if(comment){const names=fields(comment,sep);if(names.length===width)headers=names}
  }
  if(cols.length!==width)throw Error('第 '+(index+1)+' 行有 '+cols.length+' 列，应为 '+width+' 列');
  count+=width;if(count>1500000)throw Error('单个文件超过 150 万个数值');
  if(gap){rows.push([]);gap=false}
  rows.push(cols.map(v=>{const n=numeric(v)?Number(v.replace(/[dD]/,'e')):NaN;return Number.isFinite(n)?n:null}));started=true;
 }
 if(!rows.some(row=>row.filter(v=>v!==null).length>=2))throw Error('未找到同一行含两个有效数值的数据');
 return {name,rows,headers:Array.from({length:width},(_,i)=>headers[i]||'列 '+(i+1))};
}
export type Curve={id:string;table:Table;x:number;y:number;label:string};
export function runs(curve:Curve){
 const result:[number,number][][]=[];let run:[number,number][]=[];
 for(const row of curve.table.rows){const x=row[curve.x],y=row[curve.y];
  if(x==null||y==null){if(run.length){result.push(run);run=[]}}else run.push([x,y]);
 }if(run.length)result.push(run);return result;
}
// Keep local extrema when displaying dense curves. Missing rows always interrupt a line.
export function simplify(points:[number,number][],limit=2400){
 if(points.length<=limit)return points;
 const step=Math.ceil(points.length/(limit/2)),out=[points[0]];
 for(let start=1;start<points.length-1;start+=step){let lo=start,hi=start;
  for(let i=start+1;i<Math.min(start+step,points.length-1);i++){if(points[i][1]<points[lo][1])lo=i;if(points[i][1]>points[hi][1])hi=i}
  out.push(points[Math.min(lo,hi)]);if(lo!==hi)out.push(points[Math.max(lo,hi)]);
 }out.push(points[points.length-1]);return out;
}
