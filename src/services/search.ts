import { query } from './database';
import type { ObjectType, RelationTarget } from '../types';
export type SearchResult=Omit<RelationTarget,'object_type'>&{object_type:ObjectType|'section';parent_id:string|null;archived:number|null;project_name:string|null;match_location:'title'|'content';excerpt:string};
export const objectUrl = (type: string, id: string, projectId?: string | null,parentId?:string|null) => type === 'project' ? `/projects/${id}` : type === 'paper' ? `/literature/${id}` : type === 'manual' ? `/learning/${id}` : type === 'task' ? `/projects/${projectId}?tab=tasks` : type === 'calculation' ? `/data/calculations/${id}` : type === 'dataset' ? `/data/${id}` : type === 'figure' ? `/figures/${id}` : type === 'manuscript' ? `/writing/${id}` : type === 'section' ? `/writing/${parentId}?section=${id}` : `/${type === 'idea' ? 'ideas' : 'notes'}/${id}`;
type SearchOptions={type?:ObjectType|'section';projectId?:string;limit?:number;offset?:number};
const source=`(SELECT object_type,id,title,content,project_id,updated_at,NULL AS parent_id FROM knowledge_objects UNION ALL SELECT 'section',s.id,s.title,'',m.project_id,s.updated_at,s.manuscript_id FROM manuscript_sections s JOIN manuscripts m ON m.id=s.manuscript_id)`;
async function search(text:string,options:SearchOptions={}):Promise<SearchResult[]>{
 const value=text.trim().slice(0,500),fts=Array.from(value).length>=3;
 const filters=`(k.object_type!='project' OR EXISTS(SELECT 1 FROM projects WHERE id=k.id AND deleted_at IS NULL))
 AND (?='' OR k.object_type=?) AND (?='' OR (k.object_type='project' AND k.id=?) OR k.project_id=? OR EXISTS(SELECT 1 FROM project_objects po WHERE po.project_id=? AND po.object_type=k.object_type AND po.object_id=k.id))`;
 const owner=`COALESCE(k.project_id,(SELECT po.project_id FROM project_objects po JOIN projects p ON p.id=po.project_id WHERE po.object_type=k.object_type AND po.object_id=k.id AND p.deleted_at IS NULL ORDER BY p.sort_order LIMIT 1))`;
 const params:unknown[]=[value];
 let sql=`SELECT k.object_type,k.id,k.title,${owner} AS project_id,k.updated_at,k.parent_id,
 (SELECT CASE WHEN status='archived' THEN 1 ELSE 0 END FROM projects WHERE id=k.id AND k.object_type='project') AS archived,
 (SELECT name FROM projects WHERE id=${owner}) AS project_name,
 CASE WHEN instr(lower(k.title),lower(?))>0 THEN 'title' ELSE 'content' END AS match_location,`;
 if(fts){sql+=`snippet(knowledge_fts,-1,'[',']',' … ',32) AS excerpt FROM knowledge_fts JOIN ${source} k ON k.object_type=knowledge_fts.object_type AND k.id=knowledge_fts.object_id WHERE knowledge_fts MATCH ? AND ${filters} ORDER BY bm25(knowledge_fts),k.updated_at DESC,k.id LIMIT ? OFFSET ?`;params.push('"'+value.replaceAll('"','""')+'"');}
 else{sql+=`substr(CASE WHEN instr(lower(k.title),lower(?))>0 THEN k.title ELSE k.content END,1,180) AS excerpt FROM ${source} k WHERE (k.title LIKE ? ESCAPE char(92) OR k.content LIKE ? ESCAPE char(92)) AND ${filters} ORDER BY k.updated_at DESC,k.object_type,k.id LIMIT ? OFFSET ?`;const pattern='%'+value.replace(/[\\%_]/g,c=>'\\'+c)+'%';params.push(value,pattern,pattern);}
 const type=options.type||'',project=options.projectId||'';params.push(type,type,project,project,project,project,Math.min(100,Math.max(1,options.limit??100)),Math.max(0,options.offset??0));
 return query<SearchResult[]>(sql,params);
}
export const SearchService={search,searchByType:(text:string,type:ObjectType|'section',options:Omit<SearchOptions,'type'>={})=>search(text,{...options,type}),searchProject:(text:string,projectId:string,options:Omit<SearchOptions,'projectId'>={})=>search(text,{...options,projectId})};
export const searchKnowledge=SearchService.search;
