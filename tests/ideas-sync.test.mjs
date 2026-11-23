import test from 'node:test';
import assert from 'node:assert/strict';
import {IdeasClient,emptyIdea,projectPatch} from '../public/ideas-mobile/core.js';

// Unit storage adapter: browser IndexedDB persistence is verified separately in UI QA.
function client(transport,disk=new Map()){
 const c=new IdeasClient('unit',transport);c.db=true;
 c.diskWrite=async(k,v)=>disk.set(k,structuredClone(v));
 c.diskRead=async k=>structuredClone(disk.get(k));
 c.schedule=()=>{};return c;
}
function server(){
 let record={...emptyIdea('idea'),version:1,content:'original'},identity='test-space';
 const seen=new Map(),calls=[];
 const send=async(op,args)=>{
  calls.push({op,args:structuredClone(args)});
  if(op==='snapshot')return {identity,records:[record],cursor:record.version,attachments:[],projects:[],canvas:{x:0,y:0,zoom:1,version:0},limits:{attachment_bytes:20971520}};
  if(op==='mutate'){
   if(seen.has(args.op_id))return seen.get(args.op_id);
   if(args.base_version!==record.version)throw {code:'conflict',current:structuredClone(record),attempted:args.patch};
   record={...projectPatch(record,args.patch),version:record.version+1};seen.set(args.op_id,record);return record;
  }
  throw new Error('unexpected operation');
 };
 return {send,calls,get record(){return record},edit(patch){record={...record,...patch,version:record.version+1}},identity(value){identity=value}};
}
test('lost acknowledgement replays the persisted operation ID without duplicating a version',async()=>{
 const s=server(),disk=new Map();let lose=true;
 const c=client(async(op,args)=>{const result=await s.send(op,args);if(op==='mutate'&&lose){lose=false;throw new Error('lost acknowledgement')}return result},disk);
 await c.sync();await c.enqueue('idea',{content:'durable'});await c.sync();
 assert.equal(c.pending,1);assert.equal(disk.get('workspace').queue[0].sent,true);
 const op=c.state.queue[0].op_id;
 const reopened=client(s.send,disk);reopened.state=await reopened.diskRead('workspace');await reopened.sync();
 assert.equal(reopened.pending,0);assert.equal(s.record.version,2);
 assert.deepEqual(s.calls.filter(x=>x.op==='mutate').map(x=>x.args.op_id),[op,op]);
});
test('remote polling cannot silently advance the version basis of an open editor',async()=>{
 const s=server(),c=client(s.send);await c.sync();const opened=c.record('idea').version;
 s.edit({content:'remote'});await c.sync();
 await c.enqueue('idea',{content:'local'},{baseVersion:c.editVersion('idea','content',opened),beforeValues:{content:'original'}});await c.sync();
 assert.equal(c.conflicts.length,1);assert.equal(s.record.content,'remote');assert.equal(c.record('idea').content,'local');
 await c.resolve('idea','manual',{content:'remote + local'});await c.sync();assert.equal(s.record.content,'remote + local');
});
test('a dependent unsent edit promotes only when its field baseline matches the acknowledgement',async()=>{
 const s=server();let c,once=true;
 c=client(async(op,args)=>{if(op==='mutate'&&once){once=false;await c.enqueue('idea',{content:'second'},{baseVersion:1,beforeValues:{content:'first'}})}return s.send(op,args)});
 await c.sync();await c.enqueue('idea',{content:'first'});await c.sync();await c.sync();
 assert.equal(c.pending,0);assert.equal(s.record.content,'second');assert.equal(s.record.version,3);
});
test('a changed data identity keeps queued drafts and never sends them to another workspace',async()=>{
 const s=server(),c=client(s.send);await c.sync();await c.enqueue('idea',{content:'private draft'});s.identity('other-space');await c.sync();
 assert.equal(c.pending,1);assert.match(c.error,/数据空间已变化/);assert.equal(s.calls.filter(x=>x.op==='mutate').length,0);
});
test('persisted incomplete input and attachment blobs survive client reconstruction',async()=>{
 const disk=new Map(),c=client(async()=>{throw new Error('offline')},disk);
 await c.stageInput('idea','url','https://');await c.upload('idea',new File(['attachment bytes'],'notes.txt',{type:'text/plain'}));await c.sync();
 const reopened=client(async()=>{},disk);reopened.state=await reopened.diskRead('workspace');
 assert.equal(reopened.state.inputDrafts.idea.url,'https://');assert.equal(await reopened.state.uploads[0].file.text(),'attachment bytes');
 assert.equal(reopened.pending,1);
});
test('rooms and edges share durable operation replay without appearing as duplicate idea nodes',async()=>{
 const received=[],c=client(async(op,args)=>{if(op==='snapshot')return {identity:'space',records:[],cursor:0,attachments:[],projects:[],canvas:{},canvases:{hall:{x:0,y:0,zoom:1,version:0}},limits:{}};received.push(args);return {id:args.id,_entity:args.entity,version:1,...args.patch};});
 await c.enqueue('room-id',{name:'room'},{entity:'room'});await c.enqueue('edge-id',{source_idea_id:'a',target_idea_id:'b'},{entity:'edge'});
 assert.equal(c.view().length,0);assert.equal(c.view('room').length,1);assert.equal(c.view('edge').length,1);await c.sync();assert.equal(c.pending,0);assert.deepEqual(received.map(a=>a.entity),['room','edge']);
});
test('each canvas keeps its independent persisted draft and version basis',async()=>{
 const sent=[],c=client(async(op,args)=>{if(op==='canvas'){sent.push(args);return {...args,version:args.base_version+1}}return {identity:'space',records:[],cursor:0,attachments:[],projects:[],canvas:{x:0,y:0,zoom:1,version:2},canvases:{hall:{x:0,y:0,zoom:1,version:2},room:{x:100,y:100,zoom:.5,version:7}},limits:{}};});
 await c.stageCanvas({x:20,y:30,zoom:1},'hall');await c.stageCanvas({x:400,y:-50,zoom:.7},'room');await c.sync();assert.equal(c.pending,0);assert.deepEqual(sent.map(a=>[a.scope_id,a.base_version,a.x]),[['hall',2,20],['room',7,400]]);
});
test('choosing the server conflict version acknowledges a new version and archives the local draft',async()=>{
 const s=server(),c=client(s.send);await c.sync();s.edit({content:'remote'});await c.enqueue('idea',{content:'local'},{baseVersion:1});await c.sync();const before=s.record.version;await c.resolve('idea','server');await c.sync();assert.equal(s.record.content,'remote');assert.equal(s.record.version,before+1);assert.equal(c.state.resolvedDrafts[0].pending[0].patch.content,'local');
});
test('a rejected duplicate connection does not block another idea and requires an explicit decision',async()=>{
 const s=server(),c=client(async(op,args)=>{if(op==='mutate'&&args.entity==='edge')throw {code:'duplicate',message:'already connected'};return s.send(op,args)});await c.sync();
 await c.enqueue('new-edge',{source_idea_id:'a',target_idea_id:'b'},{entity:'edge'});await c.enqueue('idea',{content:'independent'});await c.sync();assert.equal(s.record.content,'independent');assert.equal(c.conflicts.length,1);await assert.rejects(c.resolve('new-edge','local'),/操作未被接受/);assert.equal(c.conflicts.length,1);await c.resolve('new-edge','server');assert.equal(c.conflicts.length,0);assert.equal(c.view('edge').filter(r=>!r.deleted_at).length,0);
});
