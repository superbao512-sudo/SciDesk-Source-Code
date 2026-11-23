import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import ts from 'typescript';
const book=JSON.parse(readFileSync(new URL('../src-tauri/learning_catalog.json',import.meta.url),'utf8'));
const source=readFileSync(new URL('../src/features/learning/learningService.ts',import.meta.url),'utf8').replace("import {projectCommand} from '../projects/projectService';",'const projectCommand=()=>{throw new Error("unexpected transport")};');
const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {filterResources,learningDirectory}=await import('data:text/javascript;base64,'+Buffer.from(compiled).toString('base64'));
test('directory covers every visible bookmark once and respects renamed and custom categories',()=>{
 const categories=book.categories.map((c,i)=>({id:`excel:${i}`,name:c.name,parent_id:''}));
 const core=book.records.filter(r=>r.decision==='是');
 const groups=learningDirectory(categories,core);
 assert.equal(groups.length,4);
 const entries=groups.flatMap(g=>g.entries);
 assert.equal(new Set(entries.map(e=>e.name)).size,entries.length);
 assert.equal(entries.reduce((n,e)=>n+e.count,0),377);
 const renamed={...categories[1],name:'我的声子资料'};
 const custom={id:'custom',name:'课堂笔记',parent_id:''};
 const changed=learningDirectory([renamed,custom],[{category1:renamed.name},{category1:custom.name}]);
 assert.equal(changed[0].entries[0].name,renamed.name);
 assert.equal(changed[1].entries[0].name,custom.name);
 const all=learningDirectory(categories,book.records.filter(r=>r.decision!=='否')).flatMap(g=>g.entries);
 assert.equal(all.reduce((n,e)=>n+e.count,0),454);
});
test('catalog reconciles every workbook row and never admits excluded records',()=>{
 assert.deepEqual(book.counts,{'否':155,'可选':77,'是':377});assert.equal(book.records.length,609);assert.equal(new Set(book.records.map(r=>r.source_id)).size,609);assert.equal(book.sheets.length,7);assert.equal(book.categories.length,18);
 assert.equal(book.records.filter(r=>r.decision==='是').length,377);assert.equal(book.records.filter(r=>r.decision==='可选').length,77);assert.equal(book.records.filter(r=>r.duplicate).length,10);assert.equal(book.records.filter(r=>r.duplicate&&r.decision!=='否').length,0);
 for(const row of book.records){assert.equal(row.url,row.raw['原始网址']);assert.equal(row.title,row.raw['书签标题']);assert.equal(row.folder,row.raw['原始文件夹路径']||'')}
 const workbook=new URL('./fixtures/synthetic-bookmarks.xlsx',import.meta.url);assert.equal(createHash('sha256').update(readFileSync(workbook)).digest('hex'),book.sha256);
});
test('inline filters search URLs, Unicode categories, properties, notes and tags with core default',()=>{
 const rows=[{title:'QE Guide',url:'https://qe.example.org/doc?x=1#s',category1:'第一性原理',category2:'声子',property:'官方文档初判',notes:'热输运',tags:'实用',tier:'core'},{title:'Community',url:'https://blog.example.org',category1:'课程',category2:'',property:'第三方教程',notes:'',tags:'',tier:'optional'}];
 for(const q of ['QE GUIDE','qe.example.org','?x=1#s','第一性原理','声子','官方文档','热输运','实用','QE 热输运'])assert.equal(filterResources(rows,q,'','','core','').length,1,q);
 assert.equal(filterResources(rows,'','','','core','').length,1);assert.equal(filterResources(rows,'','','','all','').length,2);assert.equal(filterResources(rows,'','课程','','optional','第三方教程').length,1);assert.equal(filterResources(rows,'不存在','','','all','').length,0);
});

test('title matches precede incidental mentions in a broad category',()=>{
 const rows=[{title:'PDF 翻译',url:'https://example.org/translate',category2:'LaTeX、英文写作与工具',tier:'core'},{title:'LaTeX 入门',url:'https://example.org/guide',tier:'core'}];
 assert.deepEqual(filterResources(rows,'LaTeX','','','core','').map(r=>r.title),['LaTeX 入门','PDF 翻译']);
});
