import {createContext,useContext} from 'react';
type KnowledgeObject={id:string;object_type:string;title:string};
export const KnowledgeContext=createContext<{data:{objects:KnowledgeObject[]}|null}|null>(null);
export function useKnowledge(){const store=useContext(KnowledgeContext);if(!store)throw new Error('KnowledgeProvider missing');return store;}
