import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {resolve} from 'node:path';
export function journalsBridge(root){return {name:'journals-local-service',configureServer(server){
 const token=randomBytes(32).toString('hex'),origins=[`http://127.0.0.1:${server.config.server.port}`,`http://localhost:${server.config.server.port}`];
 let port=0,failure='期刊服务正在启动';
 const child=spawn(resolve(root,'development/tauri-target/debug/journal_dev.exe'),[],{cwd:root,windowsHide:true,stdio:['pipe','pipe','pipe']});
 child.stdin.on('error',()=>{});
 child.stdin.end(JSON.stringify({control_token:token,...(process.env.SCIDESK_TEST_ROOT?{test_root:process.env.SCIDESK_TEST_ROOT}:{})})+'\n');
 let output='';child.stdout.on('data',chunk=>{output+=chunk;const end=output.indexOf('\n');if(end>=0){try{port=JSON.parse(output.slice(0,end)).port;failure=''}catch{failure='期刊服务启动响应无效'}}});
 child.stderr.on('data',chunk=>{failure=String(chunk).trim()});child.on('error',e=>{failure=e.message});child.on('exit',()=>{port=0;failure='期刊服务已停止，请重新启动开发服务'});
 server.httpServer?.once('close',()=>child.kill());process.once('exit',()=>child.kill());
 server.middlewares.use('/__scidesk/journals',(req,res)=>{
  res.setHeader('Content-Type','application/json');const fail=(error,status=503)=>{if(!res.writableEnded){res.statusCode=status;res.end(JSON.stringify({ok:false,error}))}};
  if(req.method!=='POST')return fail('仅支持应用请求',405);
  if(req.headers.origin&&!origins.includes(req.headers.origin))return fail('来源不允许',403);
  if(!port)return fail(failure);
  let body='';req.on('data',chunk=>{body+=chunk;if(Buffer.byteLength(body)>3*1024*1024)req.destroy()});
  req.on('end',()=>{void fetch(`http://127.0.0.1:${port}/command`,{method:'POST',headers:{Authorization:`Bearer ${token}`,'Content-Type':'application/json'},body}).then(async result=>{const data=await result.text();if(!res.writableEnded){res.statusCode=result.status;res.end(data)}}).catch(()=>fail('本地期刊服务连接失败，请重试'));});
 });
}}}
