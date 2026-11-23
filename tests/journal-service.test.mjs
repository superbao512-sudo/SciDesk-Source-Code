import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {DatabaseSync} from 'node:sqlite';
import ts from 'typescript';
const source=readFileSync(new URL('../src/features/journals/journalService.ts',import.meta.url),'utf8').replace(/import \{invoke,isTauri\} from '@tauri-apps\/api\/core';/, 'const invoke=(...args)=>globalThis.__journalNative(...args);const isTauri=()=>false;');
const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const service=await import('data:text/javascript;base64,'+Buffer.from(compiled).toString('base64'));
test('journal client rejects transport and persistence failures instead of returning success',async()=>{
 const original=globalThis.fetch;
 try{
  globalThis.fetch=async()=>new Response(JSON.stringify({ok:false,error:'fixture disk write failed'}),{status:200});
  await assert.rejects(service.journalService.save(null,{name:'Fixture'}),/fixture disk write failed/);
  globalThis.fetch=async()=>new Response('not JSON',{status:503});await assert.rejects(service.journalService.list(),/响应无效/);
  globalThis.fetch=async()=>{throw new Error('fixture offline')};await assert.rejects(service.journalService.feed({}),/fixture offline/);
 }finally{globalThis.fetch=original}
});
test('journal client sends real operation IDs and preserves returned persistent IDs',async()=>{
 const original=globalThis.fetch;let sent;
 try{globalThis.fetch=async(url,options)=>{assert.equal(url,'/__scidesk/journals');sent=JSON.parse(options.body);return new Response(JSON.stringify({ok:true,value:{id:'fixture-persistent-id'}}))};
 assert.deepEqual(await service.journalService.save('fixture-persistent-id',{name:'Fixture'}),{id:'fixture-persistent-id'});
 assert.deepEqual(sent,{op:'save',args:{id:'fixture-persistent-id',input:{name:'Fixture'}}});
 }finally{globalThis.fetch=original}
});
test('article links allow canonical providers and reject executable or credential URLs',()=>{
 assert.equal(service.articleLink('https://doi.org/10.5555/fixture'),'https://doi.org/10.5555/fixture');
 for(const url of ['javascript:alert(1)','file:///E:/fixture','https://doi.org.attacker.example/10.1','https://user@doi.org/x','https://doi.org:8443/x','http://doi.org/x'])assert.equal(service.articleLink(url),null);
 assert.equal(service.dateLabel('invalid date'),'日期未知');
});
test('base journal schema has no paper seeds before the requested public source catalogue is initialized',()=>{
 const db=new DatabaseSync(':memory:');try{db.exec('PRAGMA foreign_keys=ON');db.exec(readFileSync(new URL('../src-tauri/journal_migrations/001_journals.sql',import.meta.url),'utf8'));
 for(const table of ['journals','journal_subscriptions','topics','papers','push_items'])assert.equal(db.prepare(`SELECT COUNT(*) AS n FROM ${table}`).get().n,0);
 assert.deepEqual(db.prepare('PRAGMA foreign_key_check').all(),[]);assert.equal(db.prepare('PRAGMA quick_check').get().quick_check,'ok');
 assert.equal(db.prepare("SELECT COUNT(*) AS n FROM sqlite_master WHERE name LIKE 'notification%'").get().n,0);
 }finally{db.close()}
});

test('journal website buttons open HTTPS websites in browser and invoke the registered journal in desktop',async()=>{
 for(const url of ['javascript:alert(1)','file:///E:/fixture','http://journals.example.org','https://user:secret@journals.example.org'])assert.equal(service.journalWebsiteLink(url),null);
 assert.equal(service.journalWebsiteLink('https://journals.example.org'),'https://journals.example.org/');
 const originalFetch=globalThis.fetch,originalNative=globalThis.__journalNative;let opened,sent;
 try{globalThis.fetch=async(url,options)=>{assert.equal(url,'/__scidesk/journals');opened=JSON.parse(options.body);return new Response(JSON.stringify({ok:true,value:null}))};await service.openJournalWebsite({id:'registered-id',websiteUrl:'https://journals.example.org'});assert.deepEqual(opened,{op:'open_journal',args:{id:'registered-id'}});
 globalThis.__journalNative=async(...args)=>{sent=args};const nativeCompiled=compiled.replace('isTauri = () => false','isTauri = () => true');assert.notEqual(nativeCompiled,compiled);const native=await import('data:text/javascript;base64,'+Buffer.from(nativeCompiled).toString('base64'));await native.openJournalWebsite({id:'registered-id',websiteUrl:'https://journals.example.org'});assert.deepEqual(sent,['journal_command',{op:'open_journal',args:{id:'registered-id'}}]);
 }finally{globalThis.fetch=originalFetch;globalThis.__journalNative=originalNative}
});
