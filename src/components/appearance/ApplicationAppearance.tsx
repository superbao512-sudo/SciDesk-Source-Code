import {useEffect,useState,type ReactNode} from 'react';
import {getAppSettings} from '../../features/settings/settingsService';
import {settingsDefaults} from '../../features/settings/settingsModel';
import {resolveHomeBackground} from '../../features/settings/homeBackgrounds';
import {AppearanceContext,defaultBackground,type BackgroundState} from './appearanceContext';

export function ApplicationAppearance({children}:{children:ReactNode}){
 const [config,setConfig]=useState(settingsDefaults),[background,setBackground]=useState<BackgroundState>(defaultBackground);
 const [preview,setPreview]=useState({source:'',url:''});
 useEffect(()=>{let active=true,revision=0;const load=()=>{const request=++revision;void getAppSettings().then(next=>{if(active&&revision===request)setConfig(next)}).catch(()=>{/* Keep the previous appearance if settings are temporarily unavailable. */})};load();window.addEventListener('scidesk:settings-changed',load);return()=>{active=false;window.removeEventListener('scidesk:settings-changed',load)}},[]);
 const type=config['home.background_media_type'],source=config['home.background_source'],id=config['home.background_builtin_id'];
 useEffect(()=>{let active=true;void resolveHomeBackground({'home.background_media_type':type,'home.background_source':source,'home.background_builtin_id':id}).then(next=>{if(active)setBackground(next)});return()=>{active=false};
 // Only actual background choices resolve/reload media, never route, veil, accent or brand changes.
 },[type,source,id]);
 return <AppearanceContext.Provider value={{config,background,previewUrl:preview.source===background.url?preview.url:'',capturePreview:url=>setPreview({source:background.url,url}),mediaError:()=>setBackground({...defaultBackground,unavailable:true})}}>{children}</AppearanceContext.Provider>;
}
