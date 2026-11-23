import {defineConfig} from 'vite';
import {pdfAssets} from './scripts/pdf-assets';
import react from '@vitejs/plugin-react';
import {journalsBridge} from './scripts/journals-vite.mjs';
import {ideasBridge} from './scripts/ideas-vite.mjs';
import {DatabaseSync} from 'node:sqlite';
import {readFileSync,mkdirSync} from 'node:fs';
import {resolve,sep,isAbsolute} from 'node:path';
import {spawn} from 'node:child_process';
const root=import.meta.dirname;
const testRoot=process.env.SCIDESK_TEST_ROOT;
if(testRoot&&(!isAbsolute(testRoot)||!resolve(testRoot).startsWith(resolve(root,'development/verification')+sep)))throw new Error('测试数据目录必须位于第二版 development/verification 内');
const browserRoot=testRoot?resolve(testRoot):resolve(root,'runtime/browser');
export default defineConfig({plugins:[react(),pdfAssets(root),{name:'home-local-database',configureServer(server){
 const directory=resolve(browserRoot,'database');const devOrigins=[`http://127.0.0.1:${server.config.server.port}`,`http://localhost:${server.config.server.port}`];mkdirSync(directory,{recursive:true});const db=new DatabaseSync(resolve(directory,'scidesk.db'));db.exec('PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL');
 if(db.prepare('PRAGMA user_version').get()!.user_version===0){db.exec('BEGIN');try{db.exec(readFileSync(resolve(root,'src-tauri/migrations/001_home_foundation.sql'),'utf8'));db.exec('PRAGMA user_version=1; COMMIT')}catch(error){db.exec('ROLLBACK');throw error}}
 if(Number(db.prepare('PRAGMA user_version').get()!.user_version)<2){db.exec('BEGIN');try{db.exec(readFileSync(resolve(root,'src-tauri/migrations/002_project_management.sql'),'utf8'));db.exec('PRAGMA user_version=2; COMMIT')}catch(error){db.exec('ROLLBACK');throw error}}
 server.httpServer?.once('close',()=>db.close());
 server.middlewares.use('/__scidesk/projects',(req,res)=>{
  if(req.method!=='POST'){res.statusCode=405;res.end();return}
  if(req.headers.origin&&!devOrigins.includes(req.headers.origin)){res.statusCode=403;res.end();return}
  let body='';req.on('data',chunk=>{body+=chunk;if(Buffer.byteLength(body)>4194304)req.destroy()});
  req.on('end',()=>{res.setHeader('Content-Type','application/json');try{JSON.parse(body);const bridge=spawn(resolve(root,'development/tauri-target/debug/project_bridge.exe'),[resolve(directory,'scidesk.db')],{cwd:root,windowsHide:true});let output='',errorText='';bridge.stdout.on('data',chunk=>output+=chunk);bridge.stderr.on('data',chunk=>errorText+=chunk);bridge.on('error',error=>{if(!res.writableEnded){res.statusCode=500;res.end(JSON.stringify({ok:false,error:error.message}))}});bridge.on('close',code=>{if(!res.writableEnded){if(code!==0){res.statusCode=500;res.end(JSON.stringify({ok:false,error:errorText||'项目文件服务未能完成'}))}else res.end(output)}});bridge.stdin.end(body);}catch(error){res.statusCode=400;res.end(JSON.stringify({ok:false,error:String(error)}))}});
 });
 server.middlewares.use('/__scidesk/db',(req,res)=>{if(req.method!=='POST'){res.statusCode=405;res.end();return}if(req.headers.origin&&!devOrigins.includes(req.headers.origin)){res.statusCode=403;res.end();return}let body='';req.on('data',chunk=>{body+=chunk;if(body.length>65536)req.destroy()});req.on('end',()=>{res.setHeader('Content-Type','application/json');try{const {sql,values=[],select=true}=JSON.parse(body);const statement=db.prepare(sql);res.end(JSON.stringify(select?statement.all(...values):statement.run(...values),(_,value)=>typeof value==='bigint'?Number(value):value))}catch(error){res.statusCode=400;res.end(JSON.stringify({error:String(error)}))}})});
 }},ideasBridge(root),journalsBridge(root)],server:{host:'127.0.0.1',port:1421,strictPort:true,watch:{ignored:['**/runtime/**','**/development/**','**/SciDeskData/**']},fs:{deny:['.env','.env.*','*.{crt,pem}','**/.git/**','**/SciDeskData/**','**/runtime/**','**/development/**']}},build:{outDir:'dist'},clearScreen:false});
