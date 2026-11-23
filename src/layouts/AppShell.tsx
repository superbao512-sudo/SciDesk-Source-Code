import {uiText} from '../locales/language';
import {CheckUpdates} from './CheckUpdates';
import {version} from '../../package.json';
import { NavLink, useLocation } from 'react-router-dom';
import {Home,Folder,Lightbulb,BookOpen,Notebook,GraduationCap,Wrench,PanelLeftClose,PanelLeftOpen} from 'lucide-react';
import {useState,type ReactNode} from 'react';
import {ApplicationAppearance} from '../components/appearance/ApplicationAppearance';
import {useApplicationAppearance} from '../components/appearance/appearanceContext';
import {GlobalBackground} from '../components/appearance/GlobalBackground';
import {setSetting} from '../features/settings/settingsService';

export function Sidebar({collapsed,busy,onToggle}:{collapsed:boolean;busy:boolean;onToggle:()=>void}) {
  return <aside className="sidebar app-surface sidebar-surface"><button className="icon-button sidebar-toggle" aria-label={collapsed?uiText("展开侧栏"):uiText("收起侧栏")} title={collapsed?uiText("展开侧栏"):uiText("收起侧栏")} aria-expanded={!collapsed} aria-controls="main-navigation" disabled={busy} onClick={onToggle}>{collapsed?<PanelLeftOpen size={20}/>:<PanelLeftClose size={20}/>}</button><NavLink to="/" className="brand sidebar-brand" aria-label={uiText("SciDesk 首页")} title={uiText("SciDesk 首页")}><img className="sidebar-logo" src="/branding/sidebar-logo-trimmed.png" alt={uiText("SciDesk 科研 S Logo")}/><div className="sidebar-brand-copy"><img className="sidebar-wordmark" src="/branding/scidesk-sidebar-wordmark-transparent.png" alt="SciDesk"/><div className="sidebar-tagline">{uiText("个人科研工作平台")}</div></div></NavLink>
    <div className="sidebar-label">{uiText("工作空间")}</div><nav id="main-navigation" aria-label={uiText("主导航")}><NavLink to="/" end aria-label={uiText("首页")} title={uiText("首页")}><Home size={20}/><span className="sidebar-nav-text">{uiText("首页")}</span></NavLink>
      <NavLink to="/projects" aria-label={uiText("项目")} title={uiText("项目")}><Folder size={20}/><span className="sidebar-nav-text">{uiText("项目")}</span></NavLink>
      <NavLink to="/ideas" aria-label={uiText("想法")} title={uiText("想法")}><Lightbulb size={20}/><span className="sidebar-nav-text">{uiText("想法")}</span></NavLink>
      <NavLink to="/journals" aria-label={uiText("期刊")} title={uiText("期刊")}><BookOpen size={20}/><span className="sidebar-nav-text">{uiText("期刊")}</span></NavLink>
      <NavLink to="/manuals" aria-label={uiText("手册")} title={uiText("手册")}><Notebook size={20}/><span className="sidebar-nav-text">{uiText("手册")}</span></NavLink>
      <NavLink to="/learning" aria-label={uiText("学习")} title={uiText("学习")}><GraduationCap size={20}/><span className="sidebar-nav-text">{uiText("学习")}</span></NavLink>
      <NavLink to="/tools" aria-label={uiText("工具")} title={uiText("工具")}><Wrench size={20}/><span className="sidebar-nav-text">{uiText("工具")}</span></NavLink>
    </nav><CheckUpdates collapsed={collapsed}/><div className="sidebar-bottom" title={uiText("本地科研空间 · 本地优先")}><span className="local-dot"/><div>{uiText("本地科研空间")}<small>{uiText("本地优先")}</small></div><span className="version">{version}</span></div>
  </aside>;
}
type AppShellProps={children:ReactNode};
export function AppShell(props:AppShellProps){return <ApplicationAppearance><ShellContent {...props}/></ApplicationAppearance>}
function ShellContent({children}:AppShellProps){
  const location=useLocation();
  const home=location.pathname==='/';
  const {config,background,capturePreview,mediaError}=useApplicationAppearance();
  const [collapsedDraft,setCollapsedDraft]=useState<boolean|null>(null),[saving,setSaving]=useState(false),[error,setError]=useState('');
  const collapsed=collapsedDraft??config['appearance.sidebar_collapsed']??false;
  const toggle=async()=>{const next=!collapsed;setCollapsedDraft(next);setSaving(true);setError('');try{await setSetting('appearance.sidebar_collapsed',next)}catch(reason){setCollapsedDraft(null);setError(`侧栏设置保存失败：${String(reason)}`)}finally{setSaving(false)}};
  return <div className={`app-shell${home?' home-shell':' projects-shell'}${collapsed?' sidebar-collapsed':''}`}>{home&&<GlobalBackground {...background} capturePreview={capturePreview} mediaError={mediaError}/>}<Sidebar collapsed={collapsed} busy={saving} onToggle={()=>void toggle()}/><div className="workspace right-area"><main className="app-surface main-surface">{error&&<p className="shell-preference-error" role="alert">{error}</p>}<div className="page-content page-scroll-container">{children}</div></main></div></div>;
}
