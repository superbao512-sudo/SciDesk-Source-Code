// Shared desktop/mobile sync engine. IndexedDB commits precede network requests.
export function uid(){const a=new Uint8Array(16);crypto.getRandomValues(a);return Array.from(a,b=>b.toString(16).padStart(2,'0')).join('');}
export function emptyIdea(id=uid()){return {id,type:'text',title:'',content:'',url:'',tags:[],project_id:null,room_id:null,_entity:'idea',canvas_x:60,canvas_y:60,width:300,height:220,z_index:0,version:0,created_at:Date.now(),updated_at:Date.now(),deleted_at:null};}
export function field(record,key){return key==='deleted'?!!record.deleted_at:record[key];}
export function projectPatch(record,patch){const next={...record};for(const [k,v] of Object.entries(patch)){if(k==='deleted')next.deleted_at=v?Date.now():null;else next[k]=v;}return next;}
export class IdeasClient {
 constructor(name,transport){this.name=name;this.transport=transport;this.listeners=new Set();this.busy=false;this.timer=null;this.interval=null;this.writeChain=Promise.resolve();this.localWrites=0;this.localError=false;this.syncPromise=null;this.error='';this.online=false;this.failures=0;this.retryAfter=0;this.state={records:{},queue:[],uploads:[],attachments:[],cursor:0,identity:'',projects:[],canvas:{x:0,y:0,zoom:1,version:0},canvasDraft:null,receipts:{},ownVersions:{},inputDrafts:{},resolvedDrafts:[],canvases:{hall:{x:0,y:0,zoom:1,version:0}},canvasDrafts:{},limits:{attachment_bytes:20971520}};}
 async open(){this.db=await new Promise((resolve,reject)=>{const r=indexedDB.open(this.name,1);r.onupgradeneeded=()=>r.result.createObjectStore('state');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});const saved=await this.diskRead('workspace');if(saved)this.state={...this.state,...saved};if(this.state.canvasDraft){this.state.canvasDrafts.hall=this.state.canvasDraft;this.state.canvasDraft=null;}this.emit();return this;}
 diskRead(key){return new Promise((resolve,reject)=>{const t=this.db.transaction('state'),r=t.objectStore('state').get(key);r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});}
 diskWrite(key,value){return new Promise((resolve,reject)=>{const t=this.db.transaction('state','readwrite');t.objectStore('state').put(value,key);t.oncomplete=()=>resolve();t.onerror=()=>reject(t.error);t.onabort=()=>reject(t.error||new Error('本地草稿写入中止'));});}
 persist(){this.localWrites++;const snapshot=structuredClone(this.state);const result=this.writeChain.catch(()=>{}).then(()=>this.diskWrite('workspace',snapshot));this.writeChain=result;return result.then(()=>{this.localError=false}).catch(e=>{this.localError=true;this.error='本地草稿未能保存：'+String(e);throw e;}).finally(()=>{this.localWrites--;this.emit();});}
 subscribe(fn){this.listeners.add(fn);return()=>this.listeners.delete(fn);}
 emit(){for(const fn of this.listeners)fn();}
 all(){const records={...this.state.records};for(const q of this.state.queue)records[q.id]=projectPatch(records[q.id]||emptyObject(q.entity||'idea',q.id),q.patch);return Object.values(records);}
 view(entity='idea'){return this.all().filter(r=>(r._entity||'idea')===entity);}
 record(id){return this.all().find(r=>r.id===id);}
 get pending(){return this.state.queue.length+this.state.uploads.length+Object.keys(this.state.canvasDrafts).length;}
 get conflicts(){return this.state.queue.filter(q=>q.conflict);}
 get status(){return this.error?'同步失败':this.conflicts.length?'同步冲突':this.localWrites?'正在保存本地草稿':this.busy?'同步中':this.pending?'等待同步':this.online?'已同步':'离线：草稿保留在本机';}
 async stageInput(id,key,value){this.state.inputDrafts[id]||={};if(value===undefined)delete this.state.inputDrafts[id][key];else this.state.inputDrafts[id][key]=value;await this.persist();}
 editVersion(id,key,openedVersion){return Math.max(openedVersion,this.state.ownVersions[id]?.[key]||0);}
 async enqueue(id,patch,{baseVersion,beforeValues,history=true,entity,context}={}){
  const record=this.record(id)||emptyObject(entity||'idea',id),before=beforeValues||Object.fromEntries(Object.keys(patch).map(k=>[k,field(record,k)]));
  let q=this.state.queue.findLast(q=>q.id===id&&!q.sent&&!q.conflict);
  if(q&&(baseVersion===undefined||q.base_version===baseVersion)&&!context&&!q.context){for(const k of Object.keys(patch))if(!(k in q.before))q.before[k]=before[k];Object.assign(q.patch,patch);}
  else {q={op_id:uid(),id,base_version:baseVersion??record.version,patch:{...patch},before,sent:false,conflict:null,...(entity?{entity}:{}),...(context?{context}:{})};this.state.queue.push(q);}
  this.error='';this.emit();await this.persist();this.schedule();return {id,op_id:q.op_id,base_version:q.base_version,before,patch:{...patch},history,entity:q.entity,context:q.context};
 }
 async upload(ideaId,file){if(file.size>this.state.limits.attachment_bytes)throw new Error('附件超过 '+Math.floor(this.state.limits.attachment_bytes/1048576)+' MiB 限制');if(!file.size)throw new Error('不能上传空文件');const task={id:uid(),idea_id:ideaId,file,name:file.name||'剪贴板图片.png',image:file.type.startsWith('image/'),error:''};this.state.uploads.push(task);this.emit();await this.persist();this.schedule(0);return task.id;}
 async stageCanvas(canvas,scope='hall'){this.state.canvasDrafts[scope]={...canvas};this.emit();await this.persist();this.schedule();}
 schedule(delay=650){clearTimeout(this.timer);this.timer=setTimeout(()=>{this.timer=null;void this.sync();},delay);}
 start(){if(!this.interval)this.interval=setInterval(()=>void this.sync(),1600);void this.sync();}
 stop(){clearTimeout(this.timer);clearInterval(this.interval);this.timer=null;this.interval=null;}
 async flush(){clearTimeout(this.timer);this.timer=null;await this.persist();await this.sync();}
 async sync(){if(this.syncPromise)return this.syncPromise;this.syncPromise=this.performSync();try{await this.syncPromise}finally{this.syncPromise=null}}
 async performSync(){
  if(this.busy||!this.db||Date.now()<this.retryAfter)return;this.busy=true;this.emit();
  try{
   const fresh=await this.transport('snapshot',{cursor:this.state.cursor});
   if(this.state.identity&&fresh.identity!==this.state.identity)throw new Error('当前电脑的数据空间已变化，原草稿已保留；请核对连接后重新配对。');
   this.state.identity=fresh.identity;for(const r of fresh.records)this.state.records[r.id]=r;
   Object.assign(this.state,{cursor:fresh.cursor,attachments:fresh.attachments,projects:fresh.projects,canvas:fresh.canvas,canvases:fresh.canvases||{hall:fresh.canvas},limits:fresh.limits});this.online=true;this.error='';await this.persist();
   const blocked=new Set(this.conflicts.map(q=>q.id));
   for(const q of [...this.state.queue]){
    if(blocked.has(q.id))continue;
    q.sent=true;await this.persist();
    try{
     const saved=await this.transport('mutate',{id:q.id,op_id:q.op_id,base_version:q.base_version,patch:q.patch,...(q.entity?{entity:q.entity}:{}),...q.context});
     this.state.records[saved.id]=saved;this.state.receipts[q.op_id]=saved.version;this.state.ownVersions[saved.id]||={};for(const key of Object.keys(q.patch))this.state.ownVersions[saved.id][key]=saved.version;this.state.queue=this.state.queue.filter(x=>x.op_id!==q.op_id);
     // Promote only an unsent dependent draft whose field baseline is still identical.
     const next=this.state.queue.find(x=>x.id===q.id&&!x.sent&&!x.conflict);
     if(next&&Object.keys(next.patch).every(k=>JSON.stringify(field(saved,k))===JSON.stringify(next.before[k])))next.base_version=saved.version;
     await this.persist();
    }catch(e){if(e&&['conflict','room_changed','invalid','duplicate','missing'].includes(e.code)){q.conflict=e.code==='conflict'?e:{...e,rejected:true,current:this.state.records[q.id]||{...emptyObject(q.entity||'idea',q.id),deleted_at:Date.now()},attempted:q.patch};blocked.add(q.id);await this.persist();}else throw e;}
   }
   for(const upload of [...this.state.uploads]){
    if(blocked.has(upload.idea_id)||this.state.queue.some(q=>q.id===upload.idea_id))continue;
    try{const base64=await blobBase64(upload.file);await this.transport('upload',{id:upload.id,idea_id:upload.idea_id,name:upload.name,image:upload.image,base64});this.state.uploads=this.state.uploads.filter(x=>x.id!==upload.id);await this.persist();}
    catch(e){upload.error=e?.message||String(e);await this.persist();throw e;}
   }
   for(const [scope,draft] of Object.entries(this.state.canvasDrafts)){try{this.state.canvases[scope]=await this.transport('canvas',{...draft,scope_id:scope,base_version:this.state.canvases[scope]?.version||0});if(this.state.canvasDrafts[scope]===draft)delete this.state.canvasDrafts[scope];await this.persist();}catch(e){if(e?.code==='conflict'){this.state.canvases[scope]=e.current;}else throw e;}}
   if(this.pending===0){const latest=await this.transport('snapshot',{cursor:this.state.cursor});for(const r of latest.records)this.state.records[r.id]=r;this.state.attachments=latest.attachments;this.state.cursor=latest.cursor;this.state.canvases=latest.canvases||this.state.canvases;await this.persist();}this.failures=0;this.retryAfter=0;
  }catch(e){this.online=false;this.error=e?.message||String(e);this.failures++;this.retryAfter=Date.now()+Math.min(30000,1600*2**Math.min(5,this.failures));}
  finally{this.busy=false;this.emit();}
 }
 async resolve(id,choice,merged){
  const pending=this.state.queue.filter(q=>q.id===id),conflict=pending.find(q=>q.conflict)?.conflict;if(!conflict)return;
  if(choice!=='server'&&conflict.rejected)throw new Error('操作未被接受。请保留服务器状态后，依据错误提示重新操作。');
  const local=Object.assign({},...pending.map(q=>q.patch));
  this.state.resolvedDrafts=[...(this.state.resolvedDrafts||[]).slice(-99),{id,choice,pending:structuredClone(pending),resolved_at:Date.now()}];
  this.state.queue=this.state.queue.filter(q=>q.id!==id);this.state.records[id]=conflict.current;this.error='';await this.persist();
  if(!conflict.rejected)await this.enqueue(id,choice==='server'?{}:choice==='manual'?merged:{...local,...(conflict.current.deleted_at?{deleted:false}:{})},{baseVersion:conflict.current.version,entity:conflict.current._entity,context:pending[0]?.context});
  this.emit();this.schedule(0);
 }
 async retry(){this.error='';this.retryAfter=0;this.emit();await this.persist();await this.sync();}
}
export async function blobBase64(blob){return new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);reader.onerror=()=>reject(reader.error);reader.readAsDataURL(blob);});}
export function attachmentBlob(result){const b=atob(result.base64);return new Blob([Uint8Array.from(b,c=>c.charCodeAt(0))],{type:result.media_type});}
export function matching(record,query,project=''){const q=query.trim().toLocaleLowerCase();return (!project||record.project_id===project)&&(!q||[record.title,record.content,record.url,...record.tags].join('\n').toLocaleLowerCase().includes(q));}

export function validUrl(value){try{const u=new URL(value);return ['http:','https:'].includes(u.protocol)&&!u.username&&!u.password;}catch{return false;}}

export const relationTypes=['','扩展','支持','质疑','引用','验证','相关'];
export function scopeOf(record){return record._entity==='room'?'hall':record.room_id||'hall';}
export function locationName(client,record){return scopeOf(record)==='hall'?'大厅 · 未分类':client.record(scopeOf(record))?.name||'已移除包间';}
export function nodeName(record){return record?.name||record?.title||record?.content?.slice(0,40)||'未命名想法';}
export function roomMembers(client,id){return client.view().filter(r=>r.room_id===id&&!r.deleted_at).map(r=>({id:r.id,version:r.version})).sort((a,b)=>a.id<b.id?-1:a.id>b.id?1:0);}
export function edgeActive(client,edge){return !edge.deleted_at&&[edge.source_idea_id,edge.target_idea_id].every(id=>{const r=client.record(id);return r&&!r.deleted_at;});}

function emptyObject(entity,id){const r={...emptyIdea(id),_entity:entity};if(entity==='room')Object.assign(r,{name:'新包间',icon:'folder',color:'#2563eb',sort_order:0});if(entity==='edge')Object.assign(r,{source_idea_id:'',target_idea_id:'',source_anchor:'right',target_anchor:'left',line_style:'solid',color:'#8199ba',relation_type:'',label:'',directed:false});return r;}
