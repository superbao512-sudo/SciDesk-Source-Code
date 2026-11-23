import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const text=ts.transpileModule(readFileSync(new URL('../src/features/files/selection.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {selectFiles,physicalDropPoint}=await import('data:text/javascript;base64,'+Buffer.from(text).toString('base64'));
test('file selection preserves Ctrl toggles, Shift ranges and independent manual source IDs',()=>{
 const order=['sourceA:paper.pdf','sourceB:paper.pdf','folder'];assert.deepEqual(selectFiles(order,[],'sourceA:paper.pdf',null,false,false),[order[0]]);assert.deepEqual(selectFiles(order,[order[0]],order[1],order[0],true,false),order.slice(0,2));assert.deepEqual(selectFiles(order,[order[0]],'folder',order[0],false,true),order);assert.deepEqual(selectFiles(order,order,order[1],order[0],true,false),[order[0],'folder']);
});
test('native drop coordinates hit the same content point at Windows scaling levels',()=>{
 for(const scale of [1,1.25,1.5,2])assert.deepEqual(physicalDropPoint(300*scale,420*scale,scale),{x:300,y:420});
});
