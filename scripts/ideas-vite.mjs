import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {resolve} from 'node:path';
export function ideasBridge(root){
 return {name:'ideas-rust-authority',configureServer(server){
  const token=randomBytes(32).toString('hex');const allowedOrigins=[`http://127.0.0.1:${server.config.server.port}`,`http://localhost:${server.config.server.port}`];let port=0,failure='想法服务正在启动';
  const child=spawn(resolve(root,'development/tauri-target/debug/ideas_dev.exe'),[],{cwd:root,windowsHide:true,stdio:['pipe','pipe','pipe']});
  child.stdin.write(JSON.stringify({control_token:token,...(process.env.SCIDESK_TEST_ROOT?{test_root:process.env.SCIDESK_TEST_ROOT}:{})})+'\n');let output='';
  child.stdout.on('data',chunk=>{output+=chunk;const end=output.indexOf('\n');if(end>=0){try{port=JSON.parse(output.slice(0,end)).port;failure='';}catch{failure='想法本地服务启动信息无效';}}});
  child.stderr.on('data',()=>{failure='想法本地服务未就绪，请检查终端的构建结果';});
  child.on('error',error=>{failure=error.message;});child.on('exit',()=>{port=0;failure='想法本地服务已停止，请重新启动开发进程';});
  server.httpServer?.once('close',()=>child.kill());process.once('exit',()=>child.kill());
  server.middlewares.use('/__scidesk/ideas',(req,res)=>{
   res.setHeader('Content-Type','application/json');
   const fail=(message,status=503)=>{res.statusCode=status;res.end(JSON.stringify({ok:false,error:{code:'service',message}}));};
   if(req.method!=='POST')return fail('仅支持应用请求',405);
   if(req.headers.origin&&!allowedOrigins.includes(req.headers.origin))return fail('来源不允许',403);
   if(!port)return fail(failure||'想法服务尚未就绪');
   let body='',tooLarge=false;req.on('data',chunk=>{body+=chunk;if(body.length>29*1024*1024){tooLarge=true;req.destroy();}});
   req.on('end',()=>{if(tooLarge)return;void fetch(`http://127.0.0.1:${port}/command`,{method:'POST',headers:{Authorization:`Bearer ${token}`,'Content-Type':'application/json'},body,signal:AbortSignal.timeout(30000)}).then(async result=>{res.statusCode=result.status;res.end(await result.text());}).catch(()=>{if(!res.writableEnded)fail('本地想法服务连接失败，草稿已保留');});});
  });
 }};
}
