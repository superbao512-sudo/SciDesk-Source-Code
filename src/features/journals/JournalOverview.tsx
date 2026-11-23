import {uiText} from '../../locales/language';
import {useCallback,useEffect,useState} from 'react';
import {ScholarImport} from './ScholarImport';
import {UserRound,ExternalLink,RefreshCw,Plus,Check,X} from 'lucide-react';
import {journalService,dateLabel,errorMessage,openScholar} from './journalService';
import type {ScholarProfile,PaperTopic} from './types';
export function JournalOverview({revision,onChanged}:{revision:number;onChanged:()=>void}){
 const [profile,setProfile]=useState<ScholarProfile|null>(null),[avatar,setAvatar]=useState<string|null>(null),[topics,setTopics]=useState<PaperTopic[]>([]),[busy,setBusy]=useState(''),[error,setError]=useState(''),[notice,setNotice]=useState(''),[adding,setAdding]=useState(false),[newKeyword,setNewKeyword]=useState('');
 const [importing,setImporting]=useState(false);
 const importButton=<button type="button" disabled={!!busy} onClick={()=>setImporting(true)}>{uiText("导入主页 HTML")}</button>;
 const [editingLink,setEditingLink]=useState(false);
 const [profileUrl,setProfileUrl]=useState('');
 const load=useCallback(async()=>{try{const [p,t]=await Promise.all([journalService.profile(),journalService.topics()]);setProfile(p);setTopics(t);setError('')}catch(e){setError(errorMessage(e))}},[]);
 // Reads cached metadata only; remote refresh is explicit.
 // eslint-disable-next-line react-hooks/set-state-in-effect
 useEffect(()=>{void load()},[load,revision]);
 useEffect(()=>{let active=true;void journalService.avatar().then(value=>{if(active)setAvatar(value)}).catch(e=>{if(active)setError(errorMessage(e))});return()=>{active=false}},[]);
 const update=async()=>{setBusy('profile');setNotice('');setError('');try{const next=await journalService.updateProfile();setProfile(next);setNotice(next.error?'':'学术主页已更新')}catch(e){setError(errorMessage(e))}finally{setBusy('')}};
 const connect=async()=>{
  setBusy('connect');setError('');setNotice('');
  try{
   const bound=await journalService.bindProfile(profileUrl);
   setProfile(bound);setEditingLink(false);setProfileUrl('');
   const refreshed=await journalService.updateProfile();
   setProfile(refreshed);setNotice(refreshed.error?'主页链接已保存。':'学术主页已连接并更新');
  }catch(e){setError(errorMessage(e))}finally{setBusy('')}
 };
 const upload=async(file?:File)=>{if(!file)return;if(file.size>2*1024*1024||!['image/png','image/jpeg','image/webp'].includes(file.type)){setError(uiText("请选择不超过 2 MB 的 PNG、JPEG 或 WebP 图片"));return}setBusy('avatar');setError('');try{const data=await new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);reader.onerror=()=>reject(new Error('图片读取失败'));reader.readAsDataURL(file)});setAvatar(await journalService.saveAvatar(data));setNotice(uiText("头像已保存"))}catch(e){setError(errorMessage(e))}finally{setBusy('')}};
 const configured=topics.find(t=>t.name==='我的研究方向');
 const terms=configured?configured.keywords.filter(k=>k.enabled&&k.kind!=='exclude').map(k=>k.phrase):profile?.snapshot?.interests||[];
 const addKeyword=async()=>{const phrase=newKeyword.trim();if(!phrase)return;setBusy('keyword');setError('');try{const latest=await journalService.topics(),current=latest.find(t=>t.name==='我的研究方向');const keywords=current?.keywords.map(k=>({...k}))||(profile?.snapshot?.interests||[]).map(phrase=>({phrase,weight:5,field:'both' as const,kind:'boost' as const,enabled:true,notes:''}));if(keywords.some(k=>k.phrase.toLocaleLowerCase()===phrase.toLocaleLowerCase()))throw new Error('该关键词已存在');const saved=await journalService.saveTopic(current?.id||null,{name:current?.name||'我的研究方向',description:current?.description||'',enabled:current?.enabled??(keywords.length>=2),matchLogic:current?.matchLogic||'any',keywords:[...keywords,{phrase,weight:5,field:'both',kind:'boost',enabled:true}]});setTopics(previous=>previous.some(t=>t.id===saved.id)?previous.map(t=>t.id===saved.id?saved:t):[...previous,saved]);setNewKeyword('');setAdding(false);setNotice(uiText("关键词已添加"));onChanged()}catch(e){setError(errorMessage(e))}finally{setBusy('')}};
 return <div className="journal-overview"><section className="scholar-profile" aria-label={uiText("我的学术主页")}>
  {profile===null&&!error&&<p role="status">{uiText("正在读取学术主页…")}</p>}
  {profile&&(!profile.url||editingLink)&&<form className="scholar-connect" onSubmit={event=>{event.preventDefault();void connect()}}>
   <h2>{profile.url?uiText("更换学术主页"):uiText("连接学术主页")}</h2>
   <label htmlFor="scholar-profile-url">{uiText("Google Scholar 公开个人主页链接")}</label>
   <div className="journal-actions"><input id="scholar-profile-url" type="url" required placeholder="https://scholar.google.com/citations?user=…" value={profileUrl} disabled={!!busy} onChange={event=>setProfileUrl(event.target.value)}/>
    <button type="submit" disabled={!!busy||!profileUrl.trim()}>{busy==='connect'?uiText("正在连接…"):uiText("连接主页")}</button>
    {importButton}{editingLink&&<button type="button" disabled={!!busy} onClick={()=>{setEditingLink(false);setError('')}}>{uiText("取消")}</button>}
   </div>
   <p className="journal-help">{uiText("如果连接失败，请在浏览器中打开你的 Google Scholar 个人主页，在网页空白处右击，选择「另存为…」，保存为 HTML 格式。然后点击这里的「导入主页 HTML」，选择刚保存的文件。")}</p>
  </form>}
  {profile?.url&&!editingLink&&<div className="scholar-heading"><div className="scholar-identity"><label className="scholar-avatar" title={uiText("设置头像")} aria-busy={busy==='avatar'}>{avatar?<img src={avatar} alt={uiText("我的头像")}/>:<UserRound size={38}/>}<input aria-label={uiText("设置头像")} type="file" accept="image/png,image/jpeg,image/webp" disabled={!!busy} onChange={e=>{void upload(e.target.files?.[0]);e.target.value=''}}/></label><div><h2>{profile.snapshot?.name||uiText("我的学术主页")}</h2>{profile.snapshot?.affiliation&&<p>{profile.snapshot.affiliation}</p>}</div></div>
   <div className="scholar-update"><div className="journal-actions">
    <button disabled={!!busy} onClick={()=>void update()}><RefreshCw size={15} className={busy==='profile'?'journal-spin':''}/>{busy==='profile'||busy==='connect'?uiText("正在更新…"):uiText("更新主页")}</button>
    {importButton}<button onClick={()=>void openScholar().catch(e=>setError(errorMessage(e)))}><ExternalLink size={15}/>{uiText("打开 Scholar 原页")}</button>
    <button disabled={!!busy} onClick={()=>{setProfileUrl(profile.url||'');setEditingLink(true);setError('');setNotice('')}}>{uiText("更换主页")}</button>
   </div><p className="journal-help">{profile.snapshot?.source==='html'?uiText("上次本地导入"):uiText("上次更新")}：{dateLabel(profile.lastSuccessAt)}</p></div>
  </div>}
  {error&&<p role="alert" className="journal-error">{error}</p>}{notice&&<p role="status" className="journal-notice">{notice}</p>}
  {profile?.url&&!editingLink&&<>
  <div className="scholar-keywords"><h3>{uiText("研究方向")}</h3><div className="journal-terms">{terms.map(phrase=><span key={phrase}>{phrase}</span>)}{adding?<form className="journal-keyword-add" onSubmit={e=>{e.preventDefault();void addKeyword()}}><input autoFocus required maxLength={120} aria-label={uiText("新关键词")} placeholder={uiText("输入关键词，回车添加")} value={newKeyword} disabled={!!busy} onChange={e=>setNewKeyword(e.target.value)}/><button aria-label={uiText("确认添加关键词")} disabled={!!busy||!newKeyword.trim()} type="submit"><Check size={16}/></button><button type="button" aria-label={uiText("取消添加关键词")} disabled={!!busy} onClick={()=>{setAdding(false);setNewKeyword('')}}><X size={16}/></button></form>:<button className="journal-direction-add" aria-label={uiText("添加关键词")} title={uiText("添加关键词")} disabled={!!busy||!profile} onClick={()=>{setAdding(true);setNotice('')}}><Plus size={17}/></button>}</div></div>
  <section className="scholar-publications" aria-label={uiText("我的论文")}><h3>{uiText("我的论文")}</h3>{profile?.snapshot?.works.length?profile.snapshot.works.map((work,index)=><article key={index}><h3>{work.title}</h3><p>{work.authors}</p><small>{work.publication}{work.year?` · ${work.year}`:''}</small></article>):<p className="journal-help">{uiText("尚无本人论文缓存。网络可用时点击“更新主页”。")}</p>}</section>
  </>}
 {importing&&profile&&<ScholarImport profile={profile} onClose={()=>setImporting(false)} onSave={p=>{setProfile(p);setEditingLink(false);setImporting(false);setError('');setNotice(uiText("主页已从本地 HTML 导入"));onChanged()}}/>}
 </section></div>
}
