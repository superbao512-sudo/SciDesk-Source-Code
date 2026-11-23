import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
const code=ts.transpileModule(fs.readFileSync(new URL('../src/features/projects/projectLiterature.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {mergeProjectPapers,searchKeywords}=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
const row=(title,date,doi,source='crossref')=>({title,publishedAt:date,year:2025,doi,source,arxivId:null,authors:['Fixture Author'],journal:'Fixture',url:'https://doi.org/'+doi,matchedKeywords:['phonons']});
test('project search merges canonical DOI aliases and keeps one global sorted cap',()=>{
 const preprint={...row('Same science','2025-01-01','10.5555/same','arxiv'),arxivId:'2501.00001',matchedKeywords:['chiral phonons']};
 const paper=row('Same science','2025-02-01','10.5555/same');
 const merged=mergeProjectPapers([preprint,paper,row('New','2025-03-01','10.5555/new'),row('Old','2024-01-01','10.5555/old')],2,'latest');
 assert.equal(merged.length,2);assert.equal(merged[0].title,'New');assert.equal(merged[1].source,'crossref');assert.equal(merged[1].arxivId,'2501.00001');assert.deepEqual(merged[1].matchedKeywords,['chiral phonons','phonons']);
 assert.deepEqual(searchKeywords('Chiral Phonons; Phonon Heat Transport；Electride\nelectride'),['Chiral Phonons','Phonon Heat Transport','Electride']);
});
