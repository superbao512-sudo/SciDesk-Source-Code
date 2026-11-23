import {uiText} from '../../locales/language';
import {useState} from 'react';
import {useSearchParams} from 'react-router-dom';
import {BookOpen,Send,UserRound} from 'lucide-react';
import {JournalOverview} from './JournalOverview';
import {MyJournals} from './MyJournals';
import {JournalDiscovery} from './JournalDiscovery';
import './journals.css';
export function JournalsPage(){
 const [params,setParams]=useSearchParams(),mine=params.get('view')==='mine',feed=params.get('view')==='feed';
 const [visited,setVisited]=useState({overview:!mine&&!feed,feed,mine}),[revision,setRevision]=useState(0);
 const changed=()=>setRevision(value=>value+1);
 const switchTo=(view:'overview'|'feed'|'mine')=>{setVisited(previous=>({...previous,[view]:true}));setParams(view==='overview'?{}:{view},{replace:true})};
 return <div className="journals-page"><div className="journal-tabs" role="tablist" aria-label={uiText("期刊栏目")}><button id="journal-overview-tab" role="tab" aria-selected={!mine&&!feed} aria-controls="journal-overview-panel" onClick={()=>switchTo('overview')}><UserRound size={16}/>{uiText("我的学术主页")}</button><button id="journal-feed-tab" role="tab" aria-selected={feed} aria-controls="journal-feed-panel" onClick={()=>switchTo('feed')}><Send size={16}/>{uiText("期刊推送")}</button><button id="journal-mine-tab" role="tab" aria-selected={mine} aria-controls="journal-mine-panel" onClick={()=>switchTo('mine')}><BookOpen size={16}/>{uiText("期刊库")}</button></div><div id="journal-overview-panel" className="journal-tab-panel" role="tabpanel" aria-labelledby="journal-overview-tab" hidden={mine||feed}>{(visited.overview||(!mine&&!feed))&&<JournalOverview revision={revision} onChanged={changed}/>}</div><div id="journal-feed-panel" className="journal-tab-panel" role="tabpanel" aria-labelledby="journal-feed-tab" hidden={!feed}>{(visited.feed||feed)&&<JournalDiscovery active={feed} revision={revision} onChanged={changed}/>}</div><div id="journal-mine-panel" className="journal-tab-panel" role="tabpanel" aria-labelledby="journal-mine-tab" hidden={!mine}>{(visited.mine||mine)&&<MyJournals revision={revision} onChanged={changed}/>}</div></div>
}
