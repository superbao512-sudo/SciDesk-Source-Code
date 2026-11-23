import type {Box,Mark} from './model';
export function drawMark(context:CanvasRenderingContext2D,mark:Mark){
 const {x,y,w,h,color,lineWidth}=mark;context.save();context.strokeStyle=color;context.fillStyle=color;context.lineWidth=lineWidth;context.lineCap='round';context.lineJoin='round';
 switch(mark.kind){
  case 'rectangle':context.strokeRect(x,y,w,h);break;
  case 'ellipse':context.beginPath();context.ellipse(x+w/2,y+h/2,w/2,h/2,0,0,Math.PI*2);context.stroke();break;
  case 'arrow':{
   const [a,b]=mark.points,angle=Math.atan2(b.y-a.y,b.x-a.x),head=Math.max(12,lineWidth*4);
   context.beginPath();context.moveTo(a.x,a.y);context.lineTo(b.x,b.y);context.stroke();
   context.beginPath();context.moveTo(b.x,b.y);context.lineTo(b.x-head*Math.cos(angle-Math.PI/6),b.y-head*Math.sin(angle-Math.PI/6));context.lineTo(b.x-head*Math.cos(angle+Math.PI/6),b.y-head*Math.sin(angle+Math.PI/6));context.closePath();context.fill();break;
  }
  case 'pen':context.beginPath();mark.points.forEach((p,i)=>i?context.lineTo(p.x,p.y):context.moveTo(p.x,p.y));context.stroke();break;
  case 'text':{
   const size=Math.max(12,lineWidth*5+12);context.font=`${size}px "Microsoft YaHei", sans-serif`;context.textBaseline='top';
   context.beginPath();context.rect(x,y,w,h);context.clip();mark.text.split('\n').forEach((line,i)=>context.fillText(line,x,y+i*size*1.3,w));break;
  }
  case 'number':{
   context.beginPath();context.ellipse(x+w/2,y+h/2,w/2,h/2,0,0,Math.PI*2);context.fill();context.fillStyle='#fff';context.font=`600 ${Math.min(w,h)*0.62}px sans-serif`;context.textAlign='center';context.textBaseline='middle';context.fillText(String(mark.number),x+w/2,y+h/2);break;
  }
  case 'highlight':context.globalAlpha=0.3;context.fillRect(x,y,w,h);break;
  case 'mosaic':case 'blur':{
   // Sample the composited pixels beneath this object. Keep redaction editable until export.
   const left=Math.max(0,Math.floor(x)),top=Math.max(0,Math.floor(y));
   const width=Math.min(context.canvas.width-left,Math.ceil(w)),height=Math.min(context.canvas.height-top,Math.ceil(h));if(width<=0||height<=0)break;
   const temporary=document.createElement('canvas');const sample=temporary.getContext('2d')!;
   context.beginPath();context.rect(x,y,w,h);context.clip();
   if(mark.kind==='mosaic'){
    const block=Math.max(6,lineWidth*4);temporary.width=Math.max(1,Math.ceil(width/block));temporary.height=Math.max(1,Math.ceil(height/block));
    sample.drawImage(context.canvas,left,top,width,height,0,0,temporary.width,temporary.height);context.imageSmoothingEnabled=false;context.drawImage(temporary,0,0,temporary.width,temporary.height,left,top,width,height);
   }else{
    const radius=Math.max(4,lineWidth*3),padding=Math.ceil(radius*3),sx=Math.max(0,left-padding),sy=Math.max(0,top-padding);
    temporary.width=Math.min(context.canvas.width-sx,width+padding*2);temporary.height=Math.min(context.canvas.height-sy,height+padding*2);
    sample.drawImage(context.canvas,sx,sy,temporary.width,temporary.height,0,0,temporary.width,temporary.height);context.filter=`blur(${radius}px)`;context.drawImage(temporary,sx,sy);
   }break;
  }
 }context.restore();
}
export function renderScene(canvas:HTMLCanvasElement,image:HTMLImageElement,marks:Mark[]){
 if(canvas.width!==image.naturalWidth)canvas.width=image.naturalWidth;if(canvas.height!==image.naturalHeight)canvas.height=image.naturalHeight;
 const context=canvas.getContext('2d')!;context.clearRect(0,0,canvas.width,canvas.height);context.drawImage(image,0,0);marks.forEach(mark=>drawMark(context,mark));
}
export function exportScene(image:HTMLImageElement,marks:Mark[],selection:Box):string{
 // Same renderer for preview and export; crop after effects so their pixel coordinates agree.
 const scene=document.createElement('canvas');renderScene(scene,image,marks);
 const result=document.createElement('canvas');result.width=Math.max(1,Math.round(selection.w));result.height=Math.max(1,Math.round(selection.h));
 result.getContext('2d')!.drawImage(scene,Math.round(selection.x),Math.round(selection.y),result.width,result.height,0,0,result.width,result.height);
 return result.toDataURL('image/png');
}
