// Independent implementation: no WebPlotDigitizer/Engauge source is included.
export type Pixel={x:number;y:number};
export type Region={x:number;y:number;w:number;h:number};
export type AxisType='linear'|'log10'|'ln';
export type Anchor='x1'|'x2'|'y1'|'y2';
export type Calibration={xType:AxisType;yType:AxisType;pixels:Record<Anchor,Pixel|null>;values:Record<Anchor,string>};
export type DataPoint=Pixel&{id:string;source:'manual'|'auto'};
export type ExtractionOptions={color:string;tolerance:number;step:number;seed:Pixel|null};
export type Dataset={id:string;name:string;color:string;visible:boolean;points:DataPoint[];auto:ExtractionOptions;reviewed:boolean;warnings:string[];uncertain:Region[]};
export type PlotProject={schema:'scidesk-plot';version:1;image:string;imageName:string;width:number;height:number;calibration:Calibration;datasets:Dataset[];roi:Region|null;crop:Region|null};
export const defaultCalibration=():Calibration=>({xType:'linear',yType:'linear',pixels:{x1:null,x2:null,y1:null,y2:null},values:{x1:'0',x2:'100',y1:'0',y2:'100'}});
export const newDataset=(index:number):Dataset=>({id:crypto.randomUUID(),name:`数据集 ${index}`,color:['#d83b42','#2563eb','#059669','#a855f7'][index%4],visible:true,points:[],auto:{color:'#d83b42',tolerance:45,step:5,seed:null},reviewed:false,warnings:[],uncertain:[]});
export function parseNumeric(input:string):number{
 const text=input.trim();if(!text||text.length>80)throw new Error('请输入有限数值，例如 1e-5 或 10^-5');
 let value:number;
 if(/^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(text))value=Number(text);
 else {const power=/^10\^\s*(?:\(\s*([+-]?(?:\d+\.?\d*|\.\d+))\s*\)|([+-]?(?:\d+\.?\d*|\.\d+)))$/.exec(text);if(!power)throw new Error('数值格式不支持；只接受十进制、科学计数法或 10^指数');value=10**Number(power[1]??power[2]);}
 if(!Number.isFinite(value))throw new Error('数值不能是 NaN 或无穷大');
 if(value===0&&!/^[+-]?0*(?:\.0*)?(?:[eE][+-]?\d+)?$/.test(text))throw new Error('数值过小，超出可表示范围');return value;
}
const cross=(a:Pixel,b:Pixel)=>a.x*b.y-a.y*b.x;
const difference=(a:Pixel,b:Pixel):Pixel=>({x:a.x-b.x,y:a.y-b.y});
function axis(type:AxisType,first:string,second:string){
 if(!['linear','log10','ln'].includes(type))throw new Error('坐标轴类型无效');
 const a=parseNumeric(first),b=parseNumeric(second);if(a===b)throw new Error('两个刻度值不能相同');
 if(type!=='linear'&&(a<=0||b<=0))throw new Error('对数坐标的刻度值必须大于 0');
 const transform=type==='linear'?(n:number)=>n:type==='log10'?Math.log10:Math.log;
 const inverse=type==='linear'?(n:number)=>n:type==='log10'?(n:number)=>10**n:Math.exp;
 const start=transform(a),end=transform(b),range=end-start;
 if(!Number.isFinite(range)||range===0||Math.abs(range)<=Number.EPSILON*Math.max(Math.abs(start),Math.abs(end))*8)throw new Error('刻度间隔过小或跨度超出数值精度');
 return {value:(t:number)=>{const result=inverse(start+t*range);if(!Number.isFinite(result)||type!=='linear'&&result<=0)throw new Error('换算结果超出数值范围');return result;},position:(n:number)=>{if(!Number.isFinite(n)||type!=='linear'&&n<=0)throw new Error('数据值不适用于此坐标轴');return(transform(n)-start)/range;}};
}
export function createMapping(calibration:Calibration){
 const {x1,x2,y1,y2}=calibration.pixels;if(!x1||!x2||!y1||!y2)throw new Error('请先选定 X1、X2、Y1、Y2 四个标定点');
 for(const p of [x1,x2,y1,y2])if(!Number.isFinite(p.x)||!Number.isFinite(p.y))throw new Error('标定点无效');
 const u=difference(x2,x1),v=difference(y2,y1),lengthX=Math.hypot(u.x,u.y),lengthY=Math.hypot(v.x,v.y),det=cross(u,v);
 if(lengthX<5||lengthY<5)throw new Error('同轴标定点至少相隔 5 个原图像素，请选择较远刻度');
 if(Math.abs(det)/(lengthX*lengthY)<0.03)throw new Error('两轴方向近乎平行，无法可靠标定；请选择独立的 X、Y 轴刻度');
 const x=axis(calibration.xType,calibration.values.x1,calibration.values.x2),y=axis(calibration.yType,calibration.values.y1,calibration.values.y2);
 const baseY=cross(u,difference(x1,y1))/det;
 return {
  toData:(p:Pixel)=>({x:x.value(cross(difference(p,x1),v)/det),y:y.value(cross(u,difference(p,y1))/det)}),
  toPixel:(value:Pixel):Pixel=>{const tx=x.position(value.x),ty=y.position(value.y)-baseY;return {x:x1.x+u.x*tx+v.x*ty,y:x1.y+u.y*tx+v.y*ty};},
  skew:Math.abs((u.x*v.x+u.y*v.y)/(lengthX*lengthY)),
 };
}
export function formatValue(value:number,scientific=true){if(!Number.isFinite(value))throw new Error('无法导出无效数值');if(value===0)return '0';return scientific?value.toExponential(14):Number(value.toPrecision(15)).toString();}
function quote(text:string,separator:string){return text.includes(separator)||/["\r\n]/.test(text)?`"${text.replaceAll('"','""')}"`:text;}
export function exportData(project:PlotProject,ids:string[],separator= ',',scientific=true,sort=true){
 const mapping=createMapping(project.calibration),datasets=project.datasets.filter(d=>ids.includes(d.id));if(!datasets.length)throw new Error('请选择数据集');
 for(const dataset of datasets)if(dataset.points.some(p=>p.source==='auto')&&!dataset.reviewed)throw new Error(`请先复核「${dataset.name}」的自动提取结果`);
 if(!datasets.some(d=>d.points.length))throw new Error('数据集没有拾取点');
 const multiple=datasets.length>1;const lines=[multiple?['dataset_id','dataset','X','Y'].join(separator):['X','Y'].join(separator)];
 for(const dataset of datasets){const points=dataset.points.map(p=>mapping.toData(p));if(sort)points.sort((a,b)=>a.x-b.x);for(const p of points)lines.push([...(multiple?[quote(dataset.id,separator),quote(dataset.name,separator)]:[]),formatValue(p.x,scientific),formatValue(p.y,scientific)].join(separator));}return lines.join('\r\n')+'\r\n';
}
function validPixel(value:unknown,width:number,height:number):value is Pixel{const p=value as Pixel;return !!p&&Number.isFinite(p.x)&&Number.isFinite(p.y)&&p.x>=0&&p.y>=0&&p.x<=width&&p.y<=height;}
export function parseProject(text:string):PlotProject{
 if(text.length>64*1024*1024)throw new Error('项目文件超过 64 MB');
 const p=JSON.parse(text) as PlotProject;
 if(p?.schema!=='scidesk-plot'||p.version!==1||!Number.isInteger(p.width)||!Number.isInteger(p.height)||p.width<1||p.height<1||p.width*p.height>40_000_000||typeof p.image!=='string'||!/^data:image\/(png|jpeg|bmp|webp);base64,[A-Za-z0-9+/=]+$/.test(p.image))throw new Error('不是受支持的 SciDesk 曲线提取项目');
 if(typeof p.imageName!=='string'||!p.calibration||!p.calibration.pixels||!p.calibration.values)throw new Error('项目缺少图像或标定信息');
 if(!['linear','log10','ln'].includes(p.calibration.xType)||!['linear','log10','ln'].includes(p.calibration.yType))throw new Error('项目坐标轴类型无效');
 for(const name of ['x1','x2','y1','y2'] as const){const point=p.calibration.pixels[name];if(point!==null&&!validPixel(point,p.width,p.height))throw new Error('标定点越界');if(typeof p.calibration.values[name]!=='string')throw new Error('刻度值无效');}
 if(!Array.isArray(p.datasets)||p.datasets.length>50)throw new Error('数据集列表无效');
 const ids=new Set<string>();const pointIds=new Set<string>();
 for(const dataset of p.datasets){
  if(typeof dataset.id!=='string'||ids.has(dataset.id)||typeof dataset.name!=='string'||dataset.name.length>150||!/^#[0-9a-f]{6}$/i.test(dataset.color)||typeof dataset.visible!=='boolean'||!Array.isArray(dataset.points)||dataset.points.length>100000)throw new Error('数据集格式无效');ids.add(dataset.id);
  for(const point of dataset.points){if(!validPixel(point,p.width,p.height)||typeof point.id!=='string'||pointIds.has(point.id)||!['manual','auto'].includes(point.source))throw new Error('拾取点格式无效');pointIds.add(point.id);}
  if(!dataset.auto||!/^#[0-9a-f]{6}$/i.test(dataset.auto.color)||!Number.isFinite(dataset.auto.tolerance)||dataset.auto.tolerance<0||dataset.auto.tolerance>250||!Number.isInteger(dataset.auto.step)||dataset.auto.step<1||dataset.auto.step>100||dataset.auto.seed!==null&&!validPixel(dataset.auto.seed,p.width,p.height)||typeof dataset.reviewed!=='boolean'||!Array.isArray(dataset.warnings)||!dataset.warnings.every(w=>typeof w==='string')||!Array.isArray(dataset.uncertain))throw new Error('自动提取配置无效');
 }
 for(const region of [p.roi,p.crop,...p.datasets.flatMap(d=>d.uncertain)])if(region!==null&&(!validPixel(region,p.width,p.height)||!Number.isFinite(region.w)||!Number.isFinite(region.h)||region.w<1||region.h<1||region.x+region.w>p.width||region.y+region.h>p.height))throw new Error('图像区域越界');
 return p;
}
