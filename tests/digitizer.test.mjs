import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {deflateSync} from 'node:zlib';
import ts from 'typescript';
const moduleAt=async path=>import('data:text/javascript;base64,'+Buffer.from(ts.transpileModule(readFileSync(new URL(path,import.meta.url),'utf8'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText).toString('base64'));
const core=await moduleAt('../src/features/tools/digitizer/core.ts'),{extractCurve}=await moduleAt('../src/features/tools/digitizer/extract.ts');
const calibration=(xType='linear',yType='linear',values={x1:'0',x2:'100',y1:'0',y2:'100'})=>({xType,yType,values,pixels:{x1:{x:100,y:700},x2:{x:900,y:700},y1:{x:100,y:700},y2:{x:100,y:100}}});
const artifact=new URL('../development/verification/plot-digitizer/',import.meta.url);mkdirSync(artifact,{recursive:true});
const near=(actual,expected,tolerance=1e-12)=>assert.ok(Math.abs(actual-expected)<=tolerance*Math.max(Math.abs(expected),1e-300),`${actual} != ${expected}`);
test('A: Linear–Linear midpoint and nonzero origins',()=>{near(core.createMapping(calibration()).toData({x:500,y:400}).x,50);const c=calibration('linear','linear',{x1:'30',x2:'130',y1:'-25',y2:'75'});assert.deepEqual(core.createMapping(c).toData({x:500,y:400}),{x:80,y:25});});
test('B: Semi-log X midpoint is 1, not a linear mean',()=>{const m=core.createMapping(calibration('log10','linear',{x1:'1e-3',x2:'1e3',y1:'0',y2:'100'}));near(m.toData({x:500,y:400}).x,1);});
test('C: Semi-log Y midpoint is 1e-2',()=>{const m=core.createMapping(calibration('linear','log10',{x1:'0',x2:'100',y1:'1e-6',y2:'1e2'}));near(m.toData({x:500,y:400}).y,1e-2);});
test('D: Log–Log endpoints, quarters, midpoint and sixteen-decade range',()=>{
 const c=calibration('log10','log10',{x1:'10^-8',x2:'10^8',y1:'1e-6',y2:'1e2'}),m=core.createMapping(c);
 for(const t of [0,.25,.5,.75,1]){const values=m.toData({x:100+800*t,y:700-600*t});near(values.x,10**(-8+16*t));near(values.y,10**(-6+8*t));const pixel=m.toPixel(values);assert.ok(Math.hypot(pixel.x-(100+800*t),pixel.y-(700-600*t))<1e-9);}
 const ln=core.createMapping({...c,xType:'ln',yType:'ln'});for(const t of [.25,.5,.75]){const p={x:100+800*t,y:700-600*t};near(ln.toData(p).x,m.toData(p).x);near(ln.toData(p).y,m.toData(p).y);}
});
test('E: reversed linear and log axes preserve their direction',()=>{const m=core.createMapping(calibration('log10','linear',{x1:'1e8',x2:'1e-8',y1:'100',y2:'-100'}));near(m.toData({x:300,y:550}).x,1e4);near(m.toData({x:300,y:550}).y,50);});
test('F: invalid log ticks, duplicate ticks, arbitrary JS and ill-conditioned bases are rejected',()=>{
 for(const value of ['0','-1','NaN','Infinity','1e999','Math.exp(1)','1;alert(1)','10**5','1/2','1e-999']){assert.throws(()=>core.createMapping(calibration('log10','linear',{x1:value,x2:'100',y1:'0',y2:'1'})),undefined,value);}
 for(const value of ['1e-5','0.00001','10^-5','10^(-5)'])near(core.parseNumeric(value),1e-5);near(core.parseNumeric('2.5e-3'),.0025);
 assert.throws(()=>core.createMapping(calibration('linear','linear',{x1:'1',x2:'1',y1:'0',y2:'1'})),/相同/);
 const c=calibration();c.pixels.y2={x:900,y:699};assert.throws(()=>core.createMapping(c),/近乎平行/);c.pixels.x2={x:101,y:700};assert.throws(()=>core.createMapping(c),/至少相隔/);
});
test('G: rotated and skewed two-axis basis reconstructs original values independent of view scale',()=>{
 const c=calibration('log10','log10',{x1:'1e-8',x2:'1e8',y1:'1e-6',y2:'1e2'}),u={x:780,y:95},v={x:60,y:-570},origin={x:120,y:710};
 c.pixels={x1:origin,x2:{x:origin.x+u.x,y:origin.y+u.y},y1:origin,y2:{x:origin.x+v.x,y:origin.y+v.y}};
 const m=core.createMapping(c);for(const [tx,ty] of [[.25,.75],[.5,.5],[.91,.13]]){const point={x:origin.x+u.x*tx+v.x*ty,y:origin.y+u.y*tx+v.y*ty},data=m.toData(point);near(data.x,10**(-8+16*tx));near(data.y,10**(-6+8*ty));const inverse=m.toPixel(data);assert.ok(Math.hypot(inverse.x-point.x,inverse.y-point.y)<1e-9);}
 const raster={width:1100,height:900,data:new Uint8ClampedArray(1100*900*4).fill(255)},rows=['pixel_X,pixel_Y,true_X,true_Y,extracted_X,extracted_Y'];let maximumLog10Error=0;
 const draw=(p,color)=>{const x=Math.round(p.x),y=Math.round(p.y);if(x>=0&&y>=0&&x<1100&&y<900)raster.data.set([...color,255],(y*1100+x)*4)};
 for(let i=0;i<=800;i++){const t=i/800;draw({x:origin.x+u.x*t,y:origin.y+u.y*t},[80,80,80]);draw({x:origin.x+v.x*t,y:origin.y+v.y*t},[80,80,80]);}
 for(let i=0;i<=100;i++){const tx=i/100,ty=.2+.6*tx,p={x:Math.round(origin.x+u.x*tx+v.x*ty),y:Math.round(origin.y+u.y*tx+v.y*ty)},expected={x:10**(-8+16*tx),y:10**(-6+8*ty)},actual=m.toData(p);draw(p,[216,59,66]);maximumLog10Error=Math.max(maximumLog10Error,Math.abs(Math.log10(actual.x/expected.x)),Math.abs(Math.log10(actual.y/expected.y)));rows.push([p.x,p.y,expected.x,expected.y,actual.x,actual.y].join(','));}
 assert.ok(maximumLog10Error<.02);writeFileSync(new URL('tilted-calibration.png',artifact),png(raster));writeFileSync(new URL('tilted-comparison.csv',artifact),rows.join('\n'));writeFileSync(new URL('tilted-accuracy.json',artifact),JSON.stringify({maximumLog10Error}));
});
function crc32(buffer){let crc=0xffffffff;for(const byte of buffer){crc^=byte;for(let bit=0;bit<8;bit++)crc=crc&1?(crc>>>1)^0xedb88320:crc>>>1;}return (crc^0xffffffff)>>>0;}
function chunk(type,data){const name=Buffer.from(type),length=Buffer.alloc(4),crc=Buffer.alloc(4);length.writeUInt32BE(data.length);crc.writeUInt32BE(crc32(Buffer.concat([name,data])));return Buffer.concat([length,name,data,crc]);}
function png(image){const header=Buffer.alloc(13);header.writeUInt32BE(image.width,0);header.writeUInt32BE(image.height,4);header[8]=8;header[9]=6;const rows=[];for(let y=0;y<image.height;y++)rows.push(Buffer.from([0]),Buffer.from(image.data.slice(y*image.width*4,(y+1)*image.width*4)));return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(Buffer.concat(rows))),chunk('IEND',Buffer.alloc(0))]);}
function generated(){
 const width=1000,height=800,data=new Uint8ClampedArray(width*height*4).fill(255),truth=[];
 const point=(x,y,color)=>{if(x<0||y<0||x>=width||y>=height)return;data.set([...color,255],(y*width+x)*4)};
 // Neutral axes/grid and a disconnected same-color legend test the masking warnings.
 for(let x=100;x<=900;x++)for(const y of [100,250,400,550,700])point(x,y,[215,220,225]);
 for(let x=100;x<=900;x++){const t=(x-100)/800,normalized=.18+.6*(.5+.5*Math.sin(t*4)),y=Math.round(700-600*normalized);truth.push({x,t,normalized,y});for(let dy=-1;dy<=1;dy++)point(x,y+dy,[216,59,66]);}
 for(let x=300;x<370;x++)for(let y=170;y<173;y++)point(x,y,[216,59,66]);
 return {image:{width,height,data},truth};
}
test('H: rasterized known scientific curves quantify linear, semi-log and log–log errors',()=>{
 const {image,truth}=generated(),results=[];writeFileSync(new URL('known-curves.png',artifact),png(image));
 const extracted=extractCurve(image,{x:100,y:100,w:801,h:601},{color:'#d83b42',tolerance:10,step:5,seed:{x:110,y:truth[10].y}});
 assert.ok(extracted.points.length>140);assert.ok(extracted.warnings.some(w=>w.includes('未与起点连通')));
 for(const [name,xType,yType] of [['linear','linear','linear'],['semi-log-x','log10','linear'],['semi-log-y','linear','log10'],['log-log','log10','log10']]){
  const c=calibration(xType,yType,{x1:xType==='linear'?'0':'1e-8',x2:xType==='linear'?'100':'1e8',y1:yType==='linear'?'0':'1e-8',y2:yType==='linear'?'100':'1e8'}),m=core.createMapping(c);let maximumRelativeY=0,maximumRangeY=0,maximumLog10Y=0;const rows=['pixel_x,pixel_y,true_X,true_Y,extracted_X,extracted_Y'];
  for(const p of extracted.points){const t=(p.x-100)/800,normalized=.18+.6*(.5+.5*Math.sin(t*4)),actual=m.toData(p),expected=m.toData({x:p.x,y:700-600*normalized});near(actual.x,expected.x);maximumRelativeY=Math.max(maximumRelativeY,Math.abs(actual.y-expected.y)/expected.y);maximumRangeY=Math.max(maximumRangeY,Math.abs(actual.y-expected.y)/(yType==='linear'?100:1e8));maximumLog10Y=Math.max(maximumLog10Y,Math.abs(Math.log10(actual.y/expected.y)));rows.push([p.x,p.y,expected.x,expected.y,actual.x,actual.y].join(','));}
  assert.ok(yType==='linear'?maximumRangeY<.002:maximumRelativeY<.04,`${name} error too high`);results.push({name,points:extracted.points.length,maximumRelativeY,maximumRangeY,maximumLog10Y});writeFileSync(new URL(`${name}-comparison.csv`,artifact),rows.join('\n'));
 }
 writeFileSync(new URL('accuracy.json',artifact),JSON.stringify(results,null,2));console.log('Plot digitizer observed errors:',JSON.stringify(results));
 // A reopenable, real fixture also drives browser/native interaction checks.
 const p={schema:'scidesk-plot',version:1,image:'data:image/png;base64,'+png(image).toString('base64'),imageName:'known-curves.png',width:image.width,height:image.height,calibration:calibration('log10','log10',{x1:'1e-8',x2:'1e8',y1:'1e-8',y2:'1e8'}),datasets:[{...core.newDataset(1),color:'#d83b42',auto:{color:'#d83b42',tolerance:10,step:5,seed:{x:110,y:truth[10].y}},points:[],reviewed:false}],roi:{x:100,y:100,w:801,h:601},crop:null};writeFileSync(new URL('known-curves.sciplot.json',artifact),JSON.stringify(p));
});
test('automatic crossings are flagged and disconnected components are never bridged',()=>{
 const width=100,height=100,data=new Uint8ClampedArray(width*height*4).fill(255),point=(x,y)=>data.set([216,59,66,255],(y*width+x)*4);
 for(let x=10;x<90;x++){point(x,x);point(x,99-x);}const r=extractCurve({width,height,data},{x:0,y:0,w:100,h:100},{color:'#d83b42',tolerance:5,step:1,seed:{x:10,y:10}});assert.ok(r.uncertain.length>0);assert.ok(r.points.length<10);assert.ok(r.warnings.some(w=>w.includes('分叉')));
});
test('CSV contains physical values, requires automatic review and preserves raw pixels on project reopen',()=>{
 const p={schema:'scidesk-plot',version:1,image:'data:image/png;base64,AAAA',imageName:'test.png',width:1000,height:800,calibration:calibration('log10','log10',{x1:'1e-8',x2:'1e8',y1:'1e-8',y2:'1e8'}),datasets:[{...core.newDataset(1),points:[{id:'one',x:500,y:587.5,source:'manual'},{id:'two',x:300,y:400,source:'manual'}]}],roi:null,crop:null};
 const restored=core.parseProject(JSON.stringify(p));assert.deepEqual(restored.datasets[0].points,p.datasets[0].points);
 const csv=core.exportData(restored,[restored.datasets[0].id]);assert.match(csv,/1\.00000000000000e-5/);assert.ok(!csv.includes(',-5\r'));assert.ok(csv.indexOf('1.00000000000000e-4')<csv.indexOf('1.00000000000000e+0'));
 restored.datasets[0].points[0].source='auto';assert.throws(()=>core.exportData(restored,[restored.datasets[0].id]),/复核/);restored.datasets[0].reviewed=true;assert.ok(core.exportData(restored,[restored.datasets[0].id],'\t').includes('\t'));
 const next=structuredClone(restored);next.calibration.values.x2='1e4';assert.deepEqual(next.datasets[0].points,restored.datasets[0].points);assert.notEqual(core.createMapping(next.calibration).toData(next.datasets[0].points[0]).x,core.createMapping(restored.calibration).toData(restored.datasets[0].points[0]).x);
 const second={...structuredClone(restored.datasets[0]),id:'second-dataset',name:restored.datasets[0].name};restored.datasets.push(second);const combined=core.exportData(restored,restored.datasets.map(d=>d.id));assert.ok(combined.startsWith('dataset_id,dataset,X,Y'));assert.ok(combined.includes('second-dataset,'));assert.ok(combined.includes(restored.datasets[0].id+','));
});
