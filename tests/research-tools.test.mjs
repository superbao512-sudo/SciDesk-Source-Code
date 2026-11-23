import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
async function load(name){
 const src=readFileSync(new URL('../src/features/tools/research/'+name+'.ts',import.meta.url),'utf8');
 const js=ts.transpileModule(src,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
 return import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
}
const {physicsPhrase,searchTerms,termsIn}=await load('condensedMatter');
test('solid-state translation preserves thresholds, direction, citations and amount versus endpoint',()=>{
 assert.equal(physicsPhrase('The lattice thermal conductivity decreases with increasing temperature above 300 K [12].'),'当温度高于300 K时，晶格热导率随温度升高而降低[12]。');
 assert.equal(physicsPhrase('The band gap increases by 0.2 eV.'),'带隙升高了 0.2 eV。');
 assert.equal(physicsPhrase('The band gap increases to 0.2 eV.'),'带隙升高至 0.2 eV。');
 assert.equal(physicsPhrase('The band gap may increase by 0.2 eV.'),null);
 assert.equal(physicsPhrase('The band gap does not increase with increasing pressure.'),null);
 assert.equal(searchTerms('布里渊区')[0].zh,'第一布里渊区');
 assert.ok(termsIn('electron–phonon coupling').some(x=>x.zh==='电子–声子耦合'));
});
const {parseTable,runs,simplify}=await load('plotData'),{convert,groups}=await load('physics');
test('plot parser retains gaps, column identity and Fortran notation',()=>{
 const table=parseTable('spectrum.csv','"q, x",energy,label\n0,1D+1,G\n1,,G\n2,2.5,X\n\n3,0,X');
 assert.deepEqual(table.headers,['q, x','energy','label']);
 assert.deepEqual(runs({table,x:0,y:1}),[[[0,10]],[[2,2.5]],[[3,0]]]);
 assert.deepEqual(parseTable('p.dat','# T k\n1 9\n2 8').headers,['T','k']);
 assert.throws(()=>parseTable('bad.dat','1 2\n2 3 4'));
});
test('display downsampling keeps narrow extrema and endpoints',()=>{
 const input=Array.from({length:10000},(_,i)=>[i,i===7321?-50:i===7522?80:0]);
 const result=simplify(input);assert.ok(result.includes(input[7321])&&result.includes(input[7522]));
 assert.equal(result[0],input[0]);assert.equal(result.at(-1),input.at(-1));
});
test('cycles, radians, wavenumber, energy and pressure convert independently',()=>{
 const [f,p]=groups.map(g=>g.units);assert.ok(Math.abs(convert('1',f[0],f[1])-33.35640952)<1e-7);
 assert.ok(Math.abs(convert('1',f[0],f[2])-4.135667696)<1e-8);
 assert.ok(Math.abs(convert('1',f[0],f[3])-2*Math.PI)<1e-9);
 assert.equal(convert('1',p[2],p[0]),160.2176634);assert.equal(convert('1',p[0],p[1]),10);
 assert.equal(convert('1e-2',p[0],p[1]),0.1);assert.equal(convert('',p[0],p[1]),null);
 assert.equal(convert('NaN',p[0],p[1]),null);
});
