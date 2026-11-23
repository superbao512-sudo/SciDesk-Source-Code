import {uiText} from '../../locales/language';
import {useState} from 'react';
import {BookOpen} from 'lucide-react';
import logos from './journalLogos.json';
import type {PaperJournal} from './types';
export function JournalLogo({journal}:{journal:PaperJournal}){
 const [failed,setFailed]=useState(false);
 const record=logos.find(item=>item.issns.some(issn=>issn===journal.issnPrint||issn===journal.issnOnline));
 return record&&!failed?<img src={record.file} alt={uiText("{0} 标识",journal.name)} title={record.kind==='publisher'?uiText("出版社官方标识"):journal.name} onError={()=>setFailed(true)}/>:<BookOpen size={32} aria-label={uiText("期刊")}/>;
}
