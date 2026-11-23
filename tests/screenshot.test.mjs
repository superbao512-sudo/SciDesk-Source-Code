import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const compiled=ts.transpileModule(readFileSync(new URL('../src/features/tools/screenshot/model.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const model=await import('data:text/javascript;base64,'+Buffer.from(compiled).toString('base64'));
test('physical screenshot coordinates survive mixed DPI viewport scales and negative monitor origins',()=>{
 const physical={width:4480,height:1740};for(const scale of [1,1.25,1.5,2]){const p=model.imagePoint({x:2000/scale,y:800/scale},{x:0,y:0,w:4480/scale,h:1740/scale},physical);assert.ok(Math.abs(p.x-2000)<1e-9&&Math.abs(p.y-800)<1e-9);}
 assert.deepEqual(model.relativeRect({x:-1800,y:100,width:600,height:400},{x:-1920,y:-300}),{x:120,y:400,w:600,h:400});
});
test('annotation moves/resizes preserve arrow endpoints and independent undo/redo',()=>{
 const arrow=model.makeMark('arrow',{x:100,y:100},{x:200,y:200},'#ff0000',3);let history=model.commit(model.emptyHistory(),[arrow]);const moved=model.transformMark(arrow,{x:200,y:300,w:200,h:100});assert.deepEqual(moved.points,[{x:200,y:300},{x:400,y:400}]);history=model.commit(history,[moved]);assert.deepEqual(model.undo(history).present,[arrow]);assert.deepEqual(model.redo(model.undo(history)).present,[moved]);assert.deepEqual(arrow.points,[{x:100,y:100},{x:200,y:200}]);assert.equal(model.hitMark([arrow],{x:150,y:150}).id,arrow.id);assert.equal(model.hitMark([arrow],{x:100,y:200}),undefined);
});
