import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

test('project writes preserve action order across failures while directory reads remain available',async()=>{
 const calls=[];
 let failFirst;
 const gate=new Promise((_,reject)=>{failFirst=reject});
 globalThis.__projectTransport={isTauri:()=>true,invoke:async(_command,{op})=>{calls.push(op);if(op==='select')return gate;return true}};
 try{
  const source=readFileSync(new URL('../src/features/projects/projectService.ts',import.meta.url),'utf8').replace("import {invoke,isTauri} from '@tauri-apps/api/core';",'const {invoke,isTauri}=globalThis.__projectTransport;');
  const js=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
  const {projectCommand}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
  const first=projectCommand('select');
  const failed=assert.rejects(first,/isolated write failure/);
  const next=projectCommand('register');
  assert.equal(await projectCommand('list_files'),true);
  assert.ok(calls.includes('select'));assert.ok(calls.includes('list_files'));assert.ok(!calls.includes('register'));
  failFirst(new Error('isolated write failure'));await failed;assert.equal(await next,true);
  assert.equal(await projectCommand('update'),true);
  assert.ok(calls.indexOf('select')<calls.indexOf('register'));assert.ok(calls.indexOf('register')<calls.indexOf('update'));
 }finally{delete globalThis.__projectTransport}
});
