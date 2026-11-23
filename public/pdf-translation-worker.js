import {env,pipeline} from './translation-runtime/transformers.min.js';

env.allowRemoteModels=false;
env.useBrowserCache=false;
env.useFSCache=false;
env.useCustomCache=true;
env.localModelPath='/offline-models/';
env.backends.onnx.wasm.numThreads=1;
env.backends.onnx.wasm.wasmPaths=new URL('translation-runtime/',self.location.href).href;
let translator;
const baseTerms=[
 ['phonon linewidth','声子线宽'],['lattice thermal conductivity','晶格热导率'],
 ['thermal conductivity','热导率'],['phonon dispersion','声子色散'],
 ['electron–phonon coupling','电子–声子耦合'],['Brillouin zone','布里渊区'],
 ['relaxation time','弛豫时间'],['group velocity','群速度'],
];
const literals=/(\$[^$\n]+\$|\[[\d\s,–—-]+\]|(?:[A-Za-zΑ-ωκΓνħ][A-Za-zΑ-ωκΓνħ₀-₉⁰-⁹]*\s*=\s*[\wΑ-ωħ₀-₉⁰-⁹.+*/−^()-]+)|(?:[-−+]?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?\s*(?:THz|GHz|meV|eV|GPa|kbar|cm⁻¹|Å|K)\b)|[Α-ωħ]+)/gu;
self.onmessage=async({data})=>{
 try{
  if(data.files){
   const files=new Map(data.files);
   env.customCache={match:async key=>{
    const name=String(key).split('/opus-mt-en-zh/').pop();
    return files.has(name)?new Response(files.get(name)):undefined;
   },put:async()=>{}};
   try{translator=await pipeline('translation','opus-mt-en-zh',{local_files_only:true,quantized:true});}
   finally{files.clear();}
  }
  if(!translator)throw Error('模型尚未载入');
  const text=data.text.replace(/([a-z])-\s*\n\s*([a-z])/g,'$1$2').replace(/\s+/g,' ').trim();
  const terms=data.glossary?.length?data.glossary:baseTerms;
  const output=[];
  let index=0;
  for(const sentence of new Intl.Segmenter('en',{granularity:'sentence'}).segment(text)){
   const direct=data.segments?.[index++];if(direct){output.push(direct);continue;}
   if(translator.tokenizer.encode(sentence.segment).length>500)throw Error('单句过长，请分段选取后翻译');
   const [plain]=await translator(sentence.segment,{max_new_tokens:512,num_beams:4});
   let natural=plain.translation_text;
   if(/lattice thermal conductivity/i.test(sentence.segment)){
    natural=natural.replace(/(?:纬度|晶格|格子|晶体)(?:热导热率|热传导率|导热系数|热导率|导热率)/g,'晶格热导率');
   }
   const termPattern=en=>new RegExp('\\b'+en.replace(/[-–— ]/g,'[-–—\\s]+')+'(?:s)?\\b','gi');
   const relevant=terms.filter(([en])=>termPattern(en).test(sentence.segment));
   const preserved=sentence.segment.match(literals)??[];
   if(relevant.every(([,zh])=>natural.includes(zh))&&preserved.every(s=>natural.replaceAll(' ','').includes(s.replaceAll(' ','')))){
    for(const literal of preserved){if(!natural.includes(literal))natural=natural.replace(literal.replaceAll(' ',''),literal)}
    output.push(natural);continue;
   }
   const replacements=new Map();
   let marked=sentence.segment;
   function protect(value){
    let id=917001+replacements.size;
    while(text.includes(String(id))||replacements.has(String(id)))id++;
    replacements.set(String(id),value);
    return String(id);
   }
   for(const [en,zh] of terms){
    marked=marked.replace(termPattern(en),()=>protect(zh));
   }
   marked=marked.replace(literals,match=>protect(match));
   if(replacements.size&&/[a-z]{2}/i.test(marked)&&translator.tokenizer.encode(marked).length<=500){
    const [translated]=await translator(marked,{max_new_tokens:512,num_beams:2});
    if([...replacements.keys()].every(id=>translated.translation_text.includes(id))){
     output.push(translated.translation_text.replace(/\d+/g,n=>replacements.get(n)??n));continue;
    }
   }
   const parts=marked.split(/(\d+)/);
   for(const [i,part] of parts.entries()){
    if(i%2||!/[a-z]{2}/i.test(part)){output.push(replacements.get(part)??part);continue;}
    const exact=terms.find(([en])=>en.toLowerCase()===part.trim().toLowerCase());
    if(exact){output.push(exact[1]);continue;}
    if(translator.tokenizer.encode(part).length>500)throw Error('单句过长，请分段选取后翻译');
    const [result]=await translator(part,{max_new_tokens:512,num_beams:2});
    output.push(result.translation_text.replaceAll('晶格热传导率','晶格热导率').replaceAll('声子线宽度','声子线宽'));
   }
  }
  self.postMessage({result:output.join(''),terms:terms.filter(([en])=>text.toLowerCase().includes(en.toLowerCase())).map(x=>x.join(' · '))});
 }catch(e){self.postMessage({error:String(e)});}
};
