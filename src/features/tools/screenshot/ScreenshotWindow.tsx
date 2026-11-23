import {uiText} from '../../../locales/language';
import {useCallback,useEffect,useRef,useState} from 'react';
import {ArrowUpRight,Square,Circle,Pencil,Type,Hash,Highlighter,Grid2X2,Droplets,Undo2,Redo2,MousePointer2,Copy,Save,Pin,X,Scan,Monitor,AppWindow,Trash2,GripHorizontal,ChartNoAxesCombined} from 'lucide-react';
import {screenshotCommand,type CaptureFrame} from './service';
import {boxFrom,clampBox,commit,emptyHistory,hitMark,imagePoint,inside,makeMark,redo,relativeRect,transformMark,undo,type Box,type History,type Mark,type MarkKind,type Point} from './model';
import {exportScene,renderScene} from './render';
import './screenshot.css';
const tools=[['select','选择 / 移动',MousePointer2],['arrow','箭头',ArrowUpRight],['rectangle','矩形',Square],['ellipse','圆形',Circle],['pen','自由画笔',Pencil],['text','文本',Type],['number','编号',Hash],['highlight','高亮',Highlighter],['mosaic','马赛克',Grid2X2],['blur','模糊',Droplets]] as const;
type Gesture={type:'region'|'draw'|'move'|'resize'|'crop';start:Point;mark?:Mark;points?:Point[];base:History};
export function ScreenshotWindow(){
 const [frame,setFrame]=useState<CaptureFrame|null>(null),[image,setImage]=useState<HTMLImageElement|null>(null),[selection,setSelection]=useState<Box|null>(null),[history,setHistory]=useState(emptyHistory),[preview,setPreview]=useState<Mark[]|null>(null),[selected,setSelected]=useState<string|null>(null),[tool,setTool]=useState<MarkKind|'select'>('select'),[mode,setMode]=useState<'region'|'window'>('region'),[hover,setHover]=useState<Box|null>(null),[color,setColor]=useState('#ef4444'),[lineWidth,setLineWidth]=useState(3),[error,setError]=useState(''),[busy,setBusy]=useState(false),[viewport,setViewport]=useState({w:window.innerWidth,h:window.innerHeight}),[textEditor,setTextEditor]=useState<{id:string;text:string}|null>(null),[toolbar,setToolbar]=useState<Point|null>(null);
 const canvas=useRef<HTMLCanvasElement>(null),gesture=useRef<Gesture|null>(null),latest=useRef<Mark[]|null>(null);
 useEffect(()=>{let active=true;void screenshotCommand<CaptureFrame>('frame').then(value=>{if(active)setFrame(value)}).catch(e=>setError(String(e)));return()=>{active=false}},[]);
 useEffect(()=>{if(!frame)return;let active=true;const source=new Image();source.onload=()=>{if(active){setImage(source);void screenshotCommand('ready').catch(e=>setError(String(e)))}};source.onerror=()=>setError(uiText("截图图片加载失败，请按 Esc 返回"));source.src=frame.image;return()=>{active=false}},[frame]);
 useEffect(()=>{const resize=()=>setViewport({w:window.innerWidth,h:window.innerHeight});window.addEventListener('resize',resize);return()=>window.removeEventListener('resize',resize)},[]);
 useEffect(()=>{if(canvas.current&&image)renderScene(canvas.current,image,preview??history.present)},[image,preview,history.present]);
 const cancel=useCallback(()=>{if(!busy)void screenshotCommand('cancel').catch(e=>setError(String(e)))},[busy]);
 const output=useCallback(async(op:'copy'|'save'|'pin'|'digitize')=>{if(!image||!selection||busy)return;setBusy(true);setError('');try{const result=await screenshotCommand<{path?:string|null}>(op,{image:exportScene(image,history.present,selection)});if(op==='save'&&!result.path)setBusy(false)}catch(e){setError(String(e));setBusy(false)}},[image,selection,busy,history.present]);
 useEffect(()=>{
  const key=(e:KeyboardEvent)=>{
   if(e.target instanceof HTMLInputElement||e.target instanceof HTMLTextAreaElement||e.target instanceof HTMLSelectElement)return;
   if(e.key==='Escape'){e.preventDefault();cancel();return}if(busy)return;
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='a'&&frame&&!selection){e.preventDefault();setSelection({x:0,y:0,w:frame.rect.width,h:frame.rect.height});setMode('region')}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='z'){e.preventDefault();setHistory(e.shiftKey?redo:undo)}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='y'){e.preventDefault();setHistory(redo)}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='c'){e.preventDefault();void output('copy')}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'){e.preventDefault();void output('save')}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='p'){e.preventDefault();void output('pin')}
   if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='d'){e.preventDefault();void output('digitize')}
   if(e.key==='Delete'&&selected){e.preventDefault();setHistory(current=>commit(current,current.present.filter(mark=>mark.id!==selected)));setSelected(null)}
   if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)&&selected){e.preventDefault();const delta=e.shiftKey?10:1;setHistory(current=>commit(current,current.present.map(mark=>mark.id===selected?transformMark(mark,{...mark,x:mark.x+(e.key==='ArrowRight'?delta:e.key==='ArrowLeft'?-delta:0),y:mark.y+(e.key==='ArrowDown'?delta:e.key==='ArrowUp'?-delta:0)}):mark)))}
  };window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key);
 },[busy,cancel,output,selected,frame,selection]);
 const point=(event:{clientX:number;clientY:number}):Point=>{const r=canvas.current!.getBoundingClientRect();return imagePoint({x:event.clientX,y:event.clientY},{x:r.x,y:r.y,w:r.width,h:r.height},{width:frame!.rect.width,height:frame!.rect.height})};
 const sx=frame?viewport.w/frame.rect.width:1,sy=frame?viewport.h/frame.rect.height:1;
 const css=(b:Box)=>({left:b.x*sx,top:b.y*sy,width:b.w*sx,height:b.h*sy});
 const marks=preview??history.present,activeMark=marks.find(mark=>mark.id===selected);
 const showPreview=(next:Mark[])=>{latest.current=next;setPreview(next)};
 const selectWindow=async(id:number)=>{if(busy)return;setBusy(true);setError('');try{const result=await screenshotCommand<{frame:CaptureFrame;selection:{x:number;y:number;width:number;height:number}}>('window',{id});setFrame(result.frame);setSelection(relativeRect(result.selection,result.frame.rect));setHistory(emptyHistory());setSelected(null);setMode('region');setHover(null);setToolbar(null)}catch(e){setError(String(e))}finally{setBusy(false)}};
 const down=(event:React.PointerEvent<HTMLCanvasElement>)=>{
  if(event.button!==0||busy||!frame||!image)return;const p=point(event);
  if(mode==='window'){const target=frame.windows.find(w=>inside(p,relativeRect(w.rect,frame.rect)));if(target)void selectWindow(target.id);return}
  event.currentTarget.setPointerCapture(event.pointerId);latest.current=null;
  if(!selection){gesture.current={type:'region',start:p,base:history};return}
  if(!inside(p,selection))return;
  if(tool==='select'){
   if(activeMark&&Math.hypot(p.x-activeMark.x-activeMark.w,p.y-activeMark.y-activeMark.h)<12/sx){gesture.current={type:'resize',start:p,mark:activeMark,base:history};return}
   const found=hitMark(history.present,p,7/sx);setSelected(found?.id??null);
   if(found){gesture.current={type:'move',start:p,mark:found,base:history};return}
   if(Math.hypot(p.x-selection.x-selection.w,p.y-selection.y-selection.h)<14/sx)gesture.current={type:'crop',start:{x:selection.x,y:selection.y},base:history};
   return;
  }
  const number=1+history.present.filter(mark=>mark.kind==='number').reduce((max,mark)=>Math.max(max,mark.number),0);
  const mark=makeMark(tool,p,p,color,lineWidth,number);
  if(tool==='text'||tool==='number'){
   Object.assign(mark,clampBox({...mark,w:tool==='text'?220:34,h:tool==='text'?70:34},selection));
   setHistory(commit(history,[...history.present,mark]));setSelected(mark.id);if(tool==='text')setTextEditor({id:mark.id,text:''});setTool('select');return;
  }
  gesture.current={type:'draw',start:p,mark,points:[p],base:history};
 };
 const move=(event:React.PointerEvent<HTMLCanvasElement>)=>{
  if(!frame||busy)return;let p=point(event);const current=gesture.current;
  if(mode==='window'){const target=frame.windows.find(w=>inside(p,relativeRect(w.rect,frame.rect)));setHover(target?relativeRect(target.rect,frame.rect):null);return}
  if(!current)return;
  if(current.type==='region'||current.type==='crop'){setSelection(clampBox(boxFrom(current.start,p),{x:0,y:0,w:frame.rect.width,h:frame.rect.height}));return}
  if(selection)p={x:Math.max(selection.x,Math.min(selection.x+selection.w,p.x)),y:Math.max(selection.y,Math.min(selection.y+selection.h,p.y))};
  const original=current.mark!;let next=original;
  if(current.type==='draw'){
   if(original.kind==='pen'){
    current.points!.push(p);const xs=current.points!.map(v=>v.x),ys=current.points!.map(v=>v.y);next={...original,x:Math.min(...xs),y:Math.min(...ys),w:Math.max(1,Math.max(...xs)-Math.min(...xs)),h:Math.max(1,Math.max(...ys)-Math.min(...ys)),points:[...current.points!]};
   }else next={...original,...boxFrom(current.start,p),points:[current.start,p]};
   showPreview([...current.base.present,next]);
  }else{
   next=transformMark(original,current.type==='move'?{...original,x:original.x+p.x-current.start.x,y:original.y+p.y-current.start.y}:{...original,w:Math.max(2,original.w+p.x-current.start.x),h:Math.max(2,original.h+p.y-current.start.y)});
   showPreview(current.base.present.map(mark=>mark.id===original.id?next:mark));
  }
 };
 const up=(event:React.PointerEvent<HTMLCanvasElement>)=>{
  if(!gesture.current)return;const g=gesture.current;gesture.current=null;
  if(event.currentTarget.hasPointerCapture(event.pointerId))event.currentTarget.releasePointerCapture(event.pointerId);
  if(latest.current){setHistory(commit(g.base,latest.current));setSelected(g.mark?.id??null)}else if(g.type==='region'){
   const b=boxFrom(g.start,point(event));if(b.w>=3&&b.h>=3)setSelection({x:Math.round(b.x),y:Math.round(b.y),w:Math.round(b.w),h:Math.round(b.h)});else setSelection(null);
  }setPreview(null);latest.current=null;
 };
 const styleSelected=(patch:Partial<Mark>)=>{if(selected)setHistory(current=>commit(current,current.present.map(mark=>mark.id===selected?{...mark,...patch}:mark)))};
 const finishText=()=>{if(!textEditor)return;setHistory(current=>commit(current,textEditor.text.trim()?current.present.map(mark=>mark.id===textEditor.id?{...mark,text:textEditor.text}:mark):current.present.filter(mark=>mark.id!==textEditor.id)));setTextEditor(null)};
 const toolbarX=toolbar?.x??Math.max(8,Math.min(viewport.w-640,(selection?.x??0)*sx));
 const toolbarY=toolbar?.y??(selection&&((selection.y+selection.h)*sy+142<viewport.h)?(selection.y+selection.h)*sy+12:Math.max(8,(selection?.y??0)*sy-140));
 return <main className="capture-root" aria-label={uiText("高级截图选区与标注")}>
  <canvas ref={canvas} className={`capture-canvas capture-${tool}`} aria-label={uiText("拖动选择截图区域")} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={()=>{gesture.current=null;setPreview(null)}} onDoubleClick={e=>{const mark=hitMark(history.present,point(e),8/sx);if(mark?.kind==='text'){setSelected(mark.id);setTextEditor({id:mark.id,text:mark.text})}}}/>
  {!selection&&<div className="capture-start" onPointerDown={e=>e.stopPropagation()}><strong>{uiText("高级截图")}</strong><button aria-pressed={mode==='region'} onClick={()=>setMode('region')}><Scan size={16}/>{uiText("区域")}</button><button onClick={()=>{if(frame){const first=frame.monitors.find(m=>m.x<=0&&m.y<=0&&m.x+m.width>0&&m.y+m.height>0)??frame.rect;setSelection(relativeRect(first,frame.rect));setMode('region')}}}><Monitor size={16}/>{uiText("全屏")}</button>{frame&&frame.monitors.length>1&&<button onClick={()=>{setSelection({x:0,y:0,w:frame.rect.width,h:frame.rect.height});setMode('region')}}>{uiText("全部屏幕")}</button>}<button aria-pressed={mode==='window'} onClick={()=>setMode('window')}><AppWindow size={16}/>{uiText("指定窗口")}</button>{mode==='window'&&<select aria-label={uiText("选择窗口")} defaultValue="" disabled={busy} onChange={e=>void selectWindow(Number(e.target.value))}><option value="" disabled>{uiText("或在桌面点击窗口")}</option>{frame?.windows.map(w=><option key={w.id} value={w.id}>{w.title}</option>)}</select>}<span>{busy?uiText("正在捕获窗口…"):mode==='window'?uiText("点击目标窗口"):uiText("拖动框选 · Esc 取消")}</span><button aria-label={uiText("取消截图")} onClick={cancel}><X size={18}/></button></div>}
  {(selection||hover)&&<><div className="capture-dim" style={{clipPath:`polygon(0 0,100% 0,100% 100%,0 100%,0 0,${(selection??hover)!.x*sx}px ${(selection??hover)!.y*sy}px,${(selection??hover)!.x*sx}px ${((selection??hover)!.y+(selection??hover)!.h)*sy}px,${((selection??hover)!.x+(selection??hover)!.w)*sx}px ${((selection??hover)!.y+(selection??hover)!.h)*sy}px,${((selection??hover)!.x+(selection??hover)!.w)*sx}px ${(selection??hover)!.y*sy}px,${(selection??hover)!.x*sx}px ${(selection??hover)!.y*sy}px)`}}/><div className="capture-selection" style={css(selection??hover!)}><span>{Math.round((selection??hover)!.w)} × {Math.round((selection??hover)!.h)} px</span><i/></div></>}
  {!selection&&!hover&&<div className="capture-dim"/>}
  {activeMark&&<div className="capture-mark-box" style={css(activeMark)}><i/></div>}
  {textEditor&&activeMark&&<textarea className="capture-text-editor" aria-label={uiText("标注文本")} autoFocus style={{...css(activeMark),color:activeMark.color,fontSize:Math.max(12,activeMark.lineWidth*5+12)*sy}} value={textEditor.text} onChange={e=>setTextEditor({...textEditor,text:e.target.value})} onBlur={finishText} onKeyDown={e=>{if(e.key==='Escape'||e.ctrlKey&&e.key==='Enter'){e.stopPropagation();finishText()}}}/>}
  {selection&&<div className="capture-toolbar" style={{left:toolbarX,top:toolbarY}} aria-label={uiText("截图标注工具栏")}><div className="capture-toolbar-row"><span className="capture-toolbar-grip" title={uiText("拖动工具栏")} onPointerDown={e=>{const target=e.currentTarget;target.setPointerCapture(e.pointerId);const start={x:e.clientX,y:e.clientY},origin={x:toolbarX,y:toolbarY};target.onpointermove=event=>setToolbar({x:Math.max(0,Math.min(viewport.w-200,origin.x+event.clientX-start.x)),y:Math.max(0,Math.min(viewport.h-70,origin.y+event.clientY-start.y))});target.onpointerup=()=>{target.onpointermove=null};}}><GripHorizontal size={16}/></span>{tools.map(([kind,label,Icon])=><button key={kind} title={label} aria-label={label} aria-pressed={tool===kind} disabled={busy} onClick={()=>{setTool(kind);if(kind!=='select')setSelected(null)}}><Icon size={18}/></button>)}<button aria-label={uiText("撤销")} disabled={busy||!history.past.length} onClick={()=>setHistory(undo)}><Undo2 size={17}/></button><button aria-label={uiText("重做")} disabled={busy||!history.future.length} onClick={()=>setHistory(redo)}><Redo2 size={17}/></button>{selected&&<button aria-label={uiText("删除标注")} disabled={busy} onClick={()=>{setHistory(commit(history,history.present.filter(mark=>mark.id!==selected)));setSelected(null)}}><Trash2 size={17}/></button>}</div>
   <div className="capture-toolbar-row"><label>{uiText("颜色")}<input type="color" aria-label={uiText("标注颜色")} value={activeMark?.color??color} disabled={busy} onChange={e=>{setColor(e.target.value);styleSelected({color:e.target.value})}}/></label><label>{uiText("线宽")}<select aria-label={uiText("标注线宽")} value={activeMark?.lineWidth??lineWidth} disabled={busy} onChange={e=>{setLineWidth(Number(e.target.value));styleSelected({lineWidth:Number(e.target.value)})}}>{[1,2,3,5,8,12].map(w=><option key={w} value={w}>{w}</option>)}</select></label><button disabled={busy} onClick={()=>{setSelection(null);setSelected(null);setToolbar(null)}}>{uiText("重选区域")}</button><span className="capture-toolbar-spacer"/><button disabled={busy||!!textEditor} onClick={()=>void output('pin')}><Pin size={16}/>{uiText("贴图")}</button><button disabled={busy||!!textEditor} onClick={()=>void output('digitize')} title={uiText("发送截图到曲线数据提取")}><ChartNoAxesCombined size={16}/>{uiText("提取数据")}</button><button disabled={busy||!!textEditor} onClick={()=>void output('save')}><Save size={16}/>{uiText("保存")}</button><button className="capture-copy" disabled={busy||!!textEditor} onClick={()=>void output('copy')}><Copy size={16}/>{uiText("复制")}</button><button aria-label={uiText("取消截图")} disabled={busy} onClick={cancel}><X size={17}/></button></div>
   <small>{uiText("拖动标注移动 · 右下角调整大小 · 双击编辑文字 · 方向键微调")}{busy?uiText(" · 正在处理…"):''}</small></div>}
  {error&&<div role="alert" className="capture-error">{error}<button onClick={cancel}>{uiText("返回 SciDesk")}</button></div>}
 </main>;
}
