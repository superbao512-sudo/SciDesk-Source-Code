import {uiText} from '../../../locales/language';
import {useState} from 'react';
import {groups,convert} from './physics';
import './research.css';

export default function Units(){
 const [group,setGroup]=useState(0),[from,setFrom]=useState(0),[to,setTo]=useState(1),[input,setInput]=useState('1'),[status,setStatus]=useState('');
 const {units,formula}=groups[group],result=convert(input,units[from],units[to]);
 const texNumber=(n:string)=>n.replace(/e([+-]?\d+)$/i,(_,s:string)=>'\\times 10^{'+Number(s)+'}');
 const latex=result===null?'':texNumber(input.trim())+'\\,'+units[from].tex+'\\approx '+texNumber(String(result))+'\\,'+units[to].tex;
 const copy=async(text:string)=>{try{await navigator.clipboard.writeText('$'+text+'$');setStatus('LaTeX 已复制')}catch(e){setStatus(String(e))}};
 return <section className="rs-tool rs-units"><header><h1>{uiText("物理单位与公式速查")}</h1><kbd>Alt + U</kbd></header>
 <div className="rs-tabs">{groups.map((g,i)=><button key={g.label} aria-pressed={group===i} onClick={()=>{setGroup(i);setFrom(0);setTo(1);setStatus('')}}>{g.label}</button>)}</div>
 <div className="rs-fields"><label>{uiText("数值")}<input aria-label={uiText("待换算数值")} value={input} onChange={ev=>{setInput(ev.target.value);setStatus('')}}/></label>
 <label>{uiText("从")}<select value={from} onChange={ev=>setFrom(+ev.target.value)}>{units.map((u,i)=><option key={u.label} value={i}>{u.label}</option>)}</select></label>
 <button className="button" title={uiText("互换单位")} onClick={()=>{setFrom(to);setTo(from);if(result!==null)setInput(String(result))}}>⇄</button>
 <label>{uiText("到")}<select value={to} onChange={ev=>setTo(+ev.target.value)}>{units.map((u,i)=><option key={u.label} value={i}>{u.label}</option>)}</select></label></div>
 <div className="rs-result"><output>{result===null?uiText("请输入有限数值"):String(result)+' '+units[to].label}</output><button className="button" disabled={result===null} onClick={()=>void copy(latex)}>{uiText("复制换算结果 LaTeX")}</button></div>
 <div className="rs-fields"><strong>{group===0?<>E = hν = ℏω = hcν̃ = k<sub>B</sub>T，ω = 2πν</>:'1 eV/Å³ = 160.2176634 GPa = 1602.176634 kbar'}</strong><button className="button" onClick={()=>void copy(formula)}>{uiText("复制关系式 LaTeX")}</button></div>
 <small>{uiText("采用国际单位制定义常数 h、c、e、k。")}</small><p role="status">{status}</p></section>
}
