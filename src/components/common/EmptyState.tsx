import {Inbox} from 'lucide-react';
export function EmptyState({title,detail}:{title:string;detail?:string}){return <div className="empty-state"><Inbox size={28}/><strong>{title}</strong>{detail&&<p>{detail}</p>}</div>;}
