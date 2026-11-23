import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const original=readFileSync(new URL('../src/features/tools/toolService.ts',import.meta.url),'utf8').replace("import {isTauri} from '@tauri-apps/api/core';",'const isTauri=()=>globalThis.__toolNativeMode??false;').replace("import {projectCommand} from '../projects/projectService';",'const projectCommand=(...args)=>globalThis.__toolTransport(...args);');
const compiled=ts.transpileModule(original,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const service=await import('data:text/javascript;base64,'+Buffer.from(compiled).toString('base64'));
test('tool launcher opens the exact website in browser and resolves saved ID through native transport',async()=>{
 const old=globalThis.window;const mode=globalThis.__toolNativeMode;const transport=globalThis.__toolTransport;let opened,sent;
 try{globalThis.window={open:(...args)=>opened=args};globalThis.__toolNativeMode=false;await service.launchTool({id:'densityflow',url:'https://www.densityflow.com/'});assert.deepEqual(opened,['https://www.densityflow.com/','_blank','noopener,noreferrer']);globalThis.__toolNativeMode=true;globalThis.__toolTransport=async(...args)=>sent=args;await service.launchTool({id:'densityflow',url:'https://www.densityflow.com/'});assert.deepEqual(sent,['tools_open',{id:'densityflow'}]);}
 finally{globalThis.window=old;globalThis.__toolNativeMode=mode;globalThis.__toolTransport=transport}
});
test('tool URL validation prevents executable/local files, credentials and whitespace',async()=>{
 for(const value of ['javascript:alert(1)','file:///E:/private.txt','data:text/html,x','https://user:pass@example.org','https://example.org/\n',' https://example.org','not a url']){assert.equal(service.validToolUrl(value),false,value);await assert.rejects(service.launchTool({id:'bad',url:value}),/网址无效/)}assert.equal(service.validToolUrl('https://example.org/?a=1&b=2#s'),true);
});
test('failed tool saves remain failures and never return a fabricated ID',async()=>{
 const old=globalThis.__toolTransport;try{globalThis.__toolTransport=async()=>{throw new Error('fixture database unavailable')};await assert.rejects(service.toolService.save({name:'Test',description:'',url:'https://example.org'}),/database unavailable/)}finally{globalThis.__toolTransport=old}
});

test('local tools launch through the saved ID and do not attempt website navigation',async()=>{
 const old=globalThis.__toolTransport,oldWindow=globalThis.window;let sent;try{globalThis.__toolTransport=async(...args)=>{sent=args};globalThis.window={open:()=>{throw new Error('unexpected website')}};await service.launchTool({id:'registered-local',url:'',kind:'local'});assert.deepEqual(sent,['tools_open',{id:'registered-local'}]);}finally{globalThis.__toolTransport=old;globalThis.window=oldWindow}
});

test('local package preview and registration keep the exact root and confirmation token',async()=>{
 const old=globalThis.__toolTransport;const sent=[];try{globalThis.__toolTransport=async(...args)=>{sent.push(args);return {token:'verified-files',root:'E:/local tool'}};await service.toolService.previewLocal('E:/local tool');await service.toolService.registerLocal('E:/local tool','verified-files');assert.deepEqual(sent,[['tools_preview_local',{root:'E:/local tool'}],['tools_register_local',{root:'E:/local tool',token:'verified-files'}]]);}finally{globalThis.__toolTransport=old}
});
