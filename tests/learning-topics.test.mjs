import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/features/learning/resourceTopics.ts',import.meta.url),'utf8');
const {outputText} = ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}});
const {topicIndex,resourceTopics,resourcePurpose} = await import('data:text/javascript;base64,'+Buffer.from(outputText).toString('base64'));
const catalog = JSON.parse(readFileSync(new URL('../src-tauri/learning_catalog.json',import.meta.url),'utf8'));
const core = catalog.records.filter(r=>r.decision==='是');

test('each topic exposes a real starting document, tutorial and worked example from the core library',()=>{
 const index=topicIndex(core);
 for(const [topic,ids] of Object.entries({VASP:[305,306,160],'Quantum ESPRESSO':[316,292,319],'声子':[322,278,271],'热输运':[518,522,535]})){
  const entries=index.get(topic);
  for(const [i,purpose] of ['官方文档','入门教程','计算实例'].entries()){
   assert.equal(entries.find(r=>resourcePurpose(r)===purpose)?.sequence,ids[i],topic+purpose);
  }
  assert.equal(new Set(entries.map(r=>r.source_id)).size,entries.length);
 }
});
test('shared bookmarks surface across topics without copying or broad parent-category matches',()=>{
 const row=core.find(r=>r.sequence===523);
 const index=topicIndex([row]);
 for(const t of ['VASP','声子','热输运'])assert.equal(index.get(t)[0],row);
 assert.equal(index.get('Quantum ESPRESSO').length,0);
 const optional=catalog.records.filter(r=>r.decision==='可选');
 for(const entries of topicIndex(optional).values())assert.ok(entries.every(r=>r.decision==='可选'));
 assert.deepEqual(resourceTopics({title:'笔记',url:'https://example.org',category1:'声子、热输运与电子-声子耦合',category2:'其他资料',tags:''}),[]);
});
test('edited and manual bookmarks follow current metadata and third-party copies stay out of official documents',()=>{
 const manual={title:'用户笔记',url:'https://example.org/',tags:'Quantum ESPRESSO',category1:'自定义',category2:''};
 assert.deepEqual(resourceTopics(manual),['Quantum ESPRESSO']);
 assert.deepEqual(resourceTopics({...manual,tags:'热输运'}),['热输运']);
 assert.equal(resourcePurpose({...manual,title:'The VASP Manual',url:'https://www.vasp.at.example.org/wiki/index.php/The_VASP_Manual'}),'其他参考');
 assert.equal(resourcePurpose({...manual,title:'ShengBTE 报错求助'}),'其他参考');
});
