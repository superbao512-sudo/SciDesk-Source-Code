import {uiText} from '../../../locales/language';
import {useCallback,useEffect,useMemo,useRef,useState,type RefObject} from 'react';
import {useLocation} from 'react-router-dom';
import {isTauri} from '@tauri-apps/api/core';
import {projectCommand} from '../../projects/projectService';
import {useNativeFileDrop} from '../../files/useFileDragDrop';
import {SendManual,type Excerpt} from './sendManual';
import {parseTable,runs,simplify,type Curve} from './plotData';
import './research.css';
const colors=['#17679c','#c34c33','#298168','#7749a3','#a56e00','#334880','#ab4677','#307578'];
type Input={name:string;text:string};
function Chart({curves,xTitle,yTitle,title,svg}:{curves:Curve[];xTitle:string;yTitle:string;title:string;svg:RefObject<SVGSVGElement|null>}){
 const {data,bounds}=useMemo(()=>{
  const b=[Infinity,-Infinity,Infinity,-Infinity];
  const data=curves.map(c=>runs(c).map(run=>{for(const [x,y] of run){b[0]=Math.min(b[0],x);b[1]=Math.max(b[1],x);b[2]=Math.min(b[2],y);b[3]=Math.max(b[3],y)}return simplify(run)}));
  return {data,bounds:b};
 },[curves]);
 if(!Number.isFinite(bounds[0]))return <p className="rs-empty">{uiText("当前列没有成对的有效数值。")}</p>;
 const [xmin,xmax,ymin,ymax]=bounds;
 const ratio=(v:number,a:number,b:number)=>a===b?.5:(v/2-a/2)/(b/2-a/2);
 const xPos=(x:number)=>90+ratio(x,xmin,xmax)*670,yPos=(y:number)=>400-ratio(y,ymin,ymax)*325;
 const tick=(v:number)=>Number(v.toPrecision(5)).toString();
 const ticks=(lo:number,hi:number)=>lo===hi?[lo]:Array.from({length:6},(_,i)=>lo*(1-i/5)+hi*(i/5));
 const height=500+curves.length*21;
 return <svg ref={svg} xmlns="http://www.w3.org/2000/svg" viewBox={'0 0 800 '+height} width="800" height={height} role="img" aria-label={title||uiText("数据曲线")} fontFamily="Segoe UI, Microsoft YaHei, sans-serif" fontSize="14" fill="#253843">
 <rect width="800" height={height} fill="white"/><text x="425" y="40" textAnchor="middle" fontSize="21" fontWeight="600">{title}</text>
 {ticks(ymin,ymax).map((y,i)=><g key={i}><path d={'M90 '+yPos(y)+'h670'} stroke="#dce3e8"/><text x="80" y={yPos(y)+5} textAnchor="end">{tick(y)}</text></g>)}
 {ticks(xmin,xmax).map((x,i)=><g key={i}><path d={'M'+xPos(x)+' 400v6'} stroke="#384653"/><text x={xPos(x)} y="428" textAnchor="middle">{tick(x)}</text></g>)}
 <path d="M90 75V400H760" fill="none" stroke="#384653"/><text x="425" y="459" textAnchor="middle">{xTitle}</text><text transform="translate(18 238) rotate(-90)" textAnchor="middle">{yTitle}</text>
 {data.map((segments,i)=><g key={curves[i].id}>{segments.map((s,j)=>s.length===1?<circle key={j} cx={xPos(s[0][0])} cy={yPos(s[0][1])} r="3" fill={colors[i]}/>:<polyline key={j} points={s.map(([x,y])=>xPos(x).toFixed(2)+','+yPos(y).toFixed(2)).join(' ')} fill="none" stroke={colors[i]} strokeWidth="2.3"/>)}
 <path d={'M94 '+(486+i*21)+'h30'} stroke={colors[i]} strokeWidth="3"/><text x="133" y={491+i*21}>{curves[i].label}</text></g>)}</svg>
}
async function toPng(svg:string){
 const url=URL.createObjectURL(new Blob([svg],{type:'image/svg+xml;charset=utf-8'}));
 try{const im=new Image();im.src=url;await im.decode();const canvas=document.createElement('canvas');
  canvas.width=im.naturalWidth*2;canvas.height=im.naturalHeight*2;canvas.getContext('2d')!.drawImage(im,0,0,canvas.width,canvas.height);return canvas.toDataURL('image/png');
 }finally{URL.revokeObjectURL(url)}
}
export default function QuickPlot(){
 const location=useLocation(),[curves,setCurves]=useState<Curve[]>([]),[title,setTitle]=useState(''),[xTitle,setX]=useState(''),[yTitle,setY]=useState('');
 const [notice,setNotice]=useState(''),[busy,setBusy]=useState(false),[send,setSend]=useState<Excerpt|null>(null);
 const svg=useRef<SVGSVGElement>(null),area=useRef<HTMLDivElement>(null),input=useRef<HTMLInputElement>(null),initial=useRef(location.state?.source);
 const append=useCallback((files:Input[])=>{
  const additions=files.map(f=>{const table=parseTable(f.name,f.text);return {id:crypto.randomUUID(),table,x:0,y:1,label:f.name+' · '+table.headers[1]}});
  setCurves(prev=>[...prev,...additions].slice(0,8));setNotice('');
 },[]);
 useEffect(()=>{let active=true;if(initial.current)void projectCommand<Input>('research_plot',initial.current).then(file=>{if(active)append([file])}).catch(e=>{if(active)setNotice(String(e))});return()=>{active=false}},[append]);
 const action=async(fn:()=>Promise<void>)=>{setBusy(true);setNotice('');try{await fn()}catch(e){setNotice(String(e))}finally{setBusy(false)}};
 const drop=useCallback((paths:string[])=>{void action(async()=>{if(curves.length+paths.length>8)throw Error('最多叠加 8 条曲线');
  const files:Input[]=[];for(const path of paths)files.push(await projectCommand<Input>('research_plot',{path}));append(files)})},[curves.length,append]);
 useNativeFileDrop(area,!busy,drop,setNotice);
 const addFiles=(files:File[])=>void action(async()=>{if(files.length+curves.length>8)throw Error('最多叠加 8 条曲线');
  const data:Input[]=[];for(const f of files){if(f.size>8*1024*1024)throw Error('单个文件最多 8 MB');data.push({name:f.name,text:await f.text()})}append(data)});
 const change=(id:string,patch:Partial<Curve>)=>setCurves(prev=>prev.map(c=>c.id===id?{...c,...patch}:c));
 const exportAs=(format:'png'|'svg'|'manual')=>void action(async()=>{if(!svg.current)throw Error('请选择包含有效数据的列');
  const xml=new XMLSerializer().serializeToString(svg.current);
  if(format==='manual'){setSend({image:await toPng(xml),alt:title||'数据曲线',markdown:''});return}
  const data=format==='png'?(await toPng(xml)).split(',')[1]:xml;
  const path=await projectCommand<string>('research_export_plot',{format,data});setNotice('已保存至 '+path)
 });
 return <div ref={area} className="rs-tool" onDragOver={ev=>ev.preventDefault()} onDrop={ev=>{ev.preventDefault();if(!isTauri())addFiles([...ev.dataTransfer.files])}}>
 <header><h1>{uiText("科研数据快速绘图")}</h1><button className="button primary" disabled={busy||curves.length>=8} onClick={()=>input.current?.click()}>{uiText("添加数据文件")}</button><input hidden multiple type="file" ref={input} accept=".txt,.dat,.csv,.tsv" onChange={ev=>{addFiles(Array.from(ev.target.files||[]));ev.target.value=''}}/></header>
 {notice&&<p role="status" className="rs-notice">{notice}</p>}
 {!curves.length?<div className="rs-empty">{uiText("拖入 TXT、CSV、TSV 或 DAT 文件，选择 X、Y 列绘图。")}</div>:<>
 <div className="rs-fields">{[['图题',title,setTitle],['X 轴',xTitle,setX],['Y 轴',yTitle,setY]].map(([label,value,set])=><label key={String(label)}>{String(label)}<input value={String(value)} onChange={ev=>(set as (s:string)=>void)(ev.target.value)}/></label>)}</div>
 <div className="rs-plot-layout"><aside>{curves.map((c,i)=><section className="rs-curve" key={c.id}><b style={{color:colors[i]}}>{c.table.name}</b><label>{uiText("图例")}<input value={c.label} maxLength={100} onChange={ev=>change(c.id,{label:ev.target.value})}/></label>
 {(['x','y'] as const).map(axis=><label key={axis}>{axis.toUpperCase()}{uiText("列")}<select value={c[axis]} onChange={ev=>change(c.id,{[axis]:+ev.target.value,...(axis==='y'?{label:c.table.name+' · '+c.table.headers[+ev.target.value]}:{})})}>{c.table.headers.map((h,k)=><option key={k} value={k}>{k+1} · {h}</option>)}</select></label>)}
 <div className="rs-buttons"><button disabled={curves.length>=8} onClick={()=>setCurves(prev=>[...prev,{...c,id:crypto.randomUUID()}])}>{uiText("同文件加曲线")}</button><button onClick={()=>setCurves(prev=>prev.filter(v=>v.id!==c.id))}>{uiText("移除")}</button></div></section>)}</aside>
 <div className="rs-chart"><Chart curves={curves} svg={svg} title={title} xTitle={xTitle||curves[0].table.headers[curves[0].x]} yTitle={yTitle||curves[0].table.headers[curves[0].y]}/></div></div>
 <div className="rs-buttons">{(['svg','png','manual'] as const).map(f=><button className="button" key={f} disabled={busy} onClick={()=>exportAs(f)}>{f==='manual'?uiText("插入手册"):uiText("导出 ")+f.toUpperCase()}</button>)}</div></>}
 {send&&<SendManual item={send} onClose={()=>setSend(null)}/>}</div>
}
