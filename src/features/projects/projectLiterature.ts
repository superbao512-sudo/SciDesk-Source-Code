export type ProjectPaper={title:string;authors:string[];journal:string|null;publishedAt:string|null;year:number|null;doi:string|null;arxivId:string|null;url:string;source:string;matchedKeywords:string[]};
const normalize=(s:string)=>s.toLocaleLowerCase().replace(/[^\p{L}\p{N}]/gu,'');
const keys=(r:ProjectPaper)=>[r.doi?`doi:${r.doi.toLowerCase()}`:'',r.arxivId?`arxiv:${r.arxivId}`:'',`title:${normalize(r.title)}|${normalize(r.authors[0]||'')}|${r.year}`].filter(Boolean);
export function mergeProjectPapers(records:ProjectPaper[],limit:number,sort:'latest'|'relevance'){
 const merged:ProjectPaper[]=[],index=new Map<string,number>();
 for(const row of records){const aliases=keys(row),found=aliases.map(k=>index.get(k)).find(i=>i!==undefined);if(found===undefined){const i=merged.length;merged.push({...row,matchedKeywords:[...row.matchedKeywords]});aliases.forEach(k=>index.set(k,i))}else{const old=merged[found],preferred=row.source==='crossref'?row:old;merged[found]={...preferred,doi:old.doi||row.doi,arxivId:old.arxivId||row.arxivId,matchedKeywords:[...new Set([...old.matchedKeywords,...row.matchedKeywords])]};aliases.forEach(k=>index.set(k,found))}}
 return merged.sort((a,b)=>(sort==='relevance'?b.matchedKeywords.length-a.matchedKeywords.length:0)||(b.publishedAt||String(b.year||'')).localeCompare(a.publishedAt||String(a.year||''))||a.title.localeCompare(b.title)).slice(0,limit);
}
export function searchKeywords(value:string){const seen=new Set<string>();return value.split(/[\n;,，；]+/).map(v=>v.trim()).filter(v=>{const key=v.toLocaleLowerCase();if(!v||seen.has(key))return false;seen.add(key);return true})}
