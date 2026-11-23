import {useEffect,useState,type ReactNode} from 'react';
import {query} from '../services/database';
type KnowledgeObject={id:string;object_type:string;title:string};
import {KnowledgeContext} from './knowledgeContext';
export function KnowledgeProvider({children}:{children:ReactNode}){
 const [data,setData]=useState<{objects:KnowledgeObject[]}|null>(null);
 useEffect(()=>{let active=true;void query<KnowledgeObject[]>("SELECT * FROM knowledge_objects WHERE object_type='project' AND id IN (SELECT id FROM projects WHERE deleted_at IS NULL)").then(objects=>{if(active)setData({objects})}).catch(error=>console.error('Project search scope unavailable',error));return()=>{active=false}},[]);
 return <KnowledgeContext.Provider value={{data}}>{children}</KnowledgeContext.Provider>;
}
