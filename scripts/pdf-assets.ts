import type {Plugin} from 'vite';
import {readdirSync,readFileSync} from 'node:fs';
import {join} from 'node:path';

// PDF.js resolves these filenames at runtime (including CJK character maps).
export function pdfAssets(root:string):Plugin{
 const base=join(root,'node_modules/pdfjs-dist'),files=new Map<string,string>();
 for(const dir of ['cmaps','standard_fonts','iccs','wasm']){
  for(const file of readdirSync(join(base,dir)))files.set('pdf-assets/'+dir+'/'+file,join(base,dir,file));
 }
 for(const file of ['ort-wasm-simd.wasm','ort-wasm.wasm']){
  files.set('translation-runtime/'+file,join(root,'node_modules/onnxruntime-web/dist',file));
 }
 files.set('translation-runtime/transformers.min.js',join(root,'node_modules/@xenova/transformers/dist/transformers.min.js'));
 files.set('translation-runtime/TRANSFORMERS-LICENSE',join(root,'node_modules/@xenova/transformers/LICENSE'));
 files.set('pdf-assets/LICENSE',join(base,'LICENSE'));
 return {name:'offline-pdf-assets',configureServer(server){
  server.middlewares.use((req,res,next)=>{const path=files.get((req.url||'').split('?')[0].slice(1));
   if(!path){next();return}res.setHeader('Content-Type',path.endsWith('.js')?'text/javascript':path.endsWith('.wasm')?'application/wasm':'application/octet-stream');res.end(readFileSync(path))});
 },generateBundle(){for(const [fileName,path] of files)this.emitFile({type:'asset',fileName,source:readFileSync(path)})}};
}
