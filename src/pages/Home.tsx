import {uiText} from '../locales/language';
import {useEffect,useRef,useState,type CSSProperties} from 'react';
import {Pencil,Search,Sun,X} from 'lucide-react';
import {useLocation} from 'react-router-dom';
import {SearchPanel} from '../components/knowledge/SearchPanel';
import './home-inline-search.css';
import {builtinWordmark} from '../features/settings/homeBackgrounds';
import {useApplicationAppearance} from '../components/appearance/appearanceContext';
import {HOME_REFERENCE,homeCompositionStyle} from '../features/settings/homeComposition';

export type HomeProps={onEdit:(field:'subtitle'|'message')=>void};

function automaticGreeting(date:Date){const hour=date.getHours();if(hour<5)return '夜深了！';if(hour<12)return '早上好！';if(hour<18)return '下午好！';return '晚上好！'}

export function Home({onEdit}:HomeProps){
 const [text,setText]=useState(''),[composing,setComposing]=useState(false),[submitted,setSubmitted]=useState(false),searchEntry=useRef<HTMLInputElement>(null),location=useLocation();
 const routeRequest=location.state?.homeSearch?location.key:null;
 const query=text.trim();
 const clearSearch=()=>{setText('');setSubmitted(false);searchEntry.current?.focus()};
 useEffect(()=>{if(routeRequest)searchEntry.current?.focus()},[routeRequest]);
 const heroRef=useRef<HTMLElement>(null),[heroSize,setHeroSize]=useState<{width:number;height:number}>({width:HOME_REFERENCE.heroWidth,height:HOME_REFERENCE.heroHeight});
 const subtitleRef=useRef<HTMLParagraphElement>(null),[subtitleHeight,setSubtitleHeight]=useState<number>();
 useEffect(()=>{const hero=heroRef.current;if(!hero)return;const observer=new ResizeObserver(([entry])=>setHeroSize({width:entry.contentRect.width,height:entry.contentRect.height}));observer.observe(hero);return()=>observer.disconnect()},[]);
 const {config:settings,background}=useApplicationAppearance();
 const subtitleVisible=settings['home.subtitle_visible'];
 useEffect(()=>{const subtitle=subtitleRef.current;if(!subtitle)return;const observer=new ResizeObserver(([entry])=>setSubtitleHeight(entry.contentRect.height));observer.observe(subtitle);return()=>observer.disconnect()},[subtitleVisible]);
 const automatic=automaticGreeting(new Date());const greeting=settings['home.greeting_mode']==='custom'&&settings['home.custom_greeting'].trim()?settings['home.custom_greeting'].trim():uiText(automatic);
 const subtitleStyle={color:settings['home.subtitle_color'],fontFamily:`"${settings['home.subtitle_font_family']}", "STXinwei", "华文新魏", "ST Xinwei", serif`} as CSSProperties;
 const compositionStyle=homeCompositionStyle(settings['home.composition_profile'],settings['home.wordmark_style'],heroSize.width,heroSize.height,settings['home.subtitle_font_size'],settings['home.subtitle_visible'],subtitleHeight);
 return <div className="home-page" data-background={settings['home.background_builtin_id']} data-background-type={background.type} data-composition={settings['home.composition_profile']}>
  <section className="home-hero" ref={heroRef} style={compositionStyle} aria-labelledby="home-title">
   <div className="home-hero-content">
    <h1 id="home-title" className="home-wordmark"><img src={builtinWordmark(settings['home.wordmark_style'])} alt="SciDesk"/></h1>
    {settings['home.subtitle_visible']&&<p className="home-subtitle" ref={subtitleRef} style={subtitleStyle}><button className="home-subtitle-edit" type="button" title={uiText("编辑首页副标题")} aria-label={uiText("编辑首页副标题")} onClick={()=>onEdit('subtitle')}>{(settings['home.subtitle_text']==='你的个人科研工作平台'?uiText('你的个人科研工作平台'):settings['home.subtitle_text'])||<Pencil size={16}/>}</button></p>}
    <div className="home-search-content">
    <form className="home-hero-search" role="search" onSubmit={event=>{event.preventDefault();if(!composing)setSubmitted(true)}}><Search aria-hidden="true"/><input ref={searchEntry} type="search" aria-label={uiText("首页全局搜索")} aria-controls={query&&!composing?'home-search-results':undefined} placeholder={uiText("搜索项目、文献、数据、笔记、计算记录……")} value={text} maxLength={500} autoComplete="off" onChange={event=>{setText(event.target.value);setSubmitted(false)}} onCompositionStart={()=>setComposing(true)} onCompositionEnd={()=>setComposing(false)} onKeyDown={event=>{if(event.key==='Escape'&&!event.nativeEvent.isComposing){event.preventDefault();clearSearch()}}}/>{text&&<button type="button" className="home-search-clear" aria-label={uiText("清空搜索")} onClick={clearSearch}><X size={18}/></button>}<kbd>Ctrl K</kbd></form>
    {query&&!composing&&<SearchPanel query={query} immediate={submitted} onNavigate={clearSearch}/>}
    <div className="home-greeting"><h2>{greeting}<Sun className="home-greeting-sun" aria-hidden="true"/></h2><div className="home-message"><p>{settings['home.message']==='专注于重要的科学问题，一步一个脚印。'?uiText(settings['home.message']):settings['home.message']}</p><button type="button" aria-label={uiText("编辑首页个性文字")} onClick={()=>onEdit('message')}><Pencil/></button></div></div>
    </div>
   </div>
  </section>
 </div>;
}
