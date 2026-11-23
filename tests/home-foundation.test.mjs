import test from 'node:test';
import assert from 'node:assert/strict';
import {DatabaseSync} from 'node:sqlite';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const schema=readFileSync(new URL('../src-tauri/migrations/001_home_foundation.sql',import.meta.url),'utf8');
function empty(){const db=new DatabaseSync(':memory:');db.exec('PRAGMA foreign_keys=ON');db.exec(schema);return db;}
test('fresh metadata database has no business records or seeded preferences',()=>{const db=empty();try{for(const table of ['projects','tasks','ideas','papers','notes','calculations','datasets','manuals','figures','manuscripts','settings'])assert.equal(db.prepare('SELECT count(*) AS n FROM '+table).get().n,0);assert.equal(db.prepare('PRAGMA quick_check').get().quick_check,'ok');assert.deepEqual(db.prepare('PRAGMA foreign_key_check').all(),[]);}finally{db.close()}});
test('original SearchService executes LIKE, FTS, filters and deleted-project exclusion',async()=>{const db=empty();try{
 db.prepare("INSERT INTO projects(id,name) VALUES(?,?)").run('test-project','声子研究');
 db.prepare("INSERT INTO notes(id,title,content,project_id) VALUES(?,?,?,?)").run('test-note','输运笔记','phonon thermal transport','test-project');
 const source=readFileSync(new URL('../src/services/search.ts',import.meta.url),'utf8').replace(/import \{ query \} from '.\/database';/, 'const query=globalThis.__homeTestQuery;');
 globalThis.__homeTestQuery=async(sql,params=[])=>db.prepare(sql).all(...params);
 const js=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
 const {SearchService}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
 assert.equal((await SearchService.search('声子'))[0].id,'test-project');
 assert.equal((await SearchService.search('phonon'))[0].id,'test-note');
 assert.equal((await SearchService.search('phonon',{type:'paper'})).length,0);
 assert.equal((await SearchService.search('phonon',{projectId:'test-project'})).length,1);
 db.exec("UPDATE projects SET deleted_at='2026-10-22' WHERE id='test-project'");
 assert.equal((await SearchService.search('声子')).length,0);
 }finally{delete globalThis.__homeTestQuery;db.close()}});
test('home settings writes preserve other settings',()=>{const db=empty();try{const write=db.prepare('INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value');write.run('home.message','one');write.run('home.subtitle_text','subtitle');write.run('home.message','two');assert.equal(db.prepare("SELECT value FROM settings WHERE key='home.message'").get().value,'two');assert.equal(db.prepare("SELECT value FROM settings WHERE key='home.subtitle_text'").get().value,'subtitle');}finally{db.close()}});
