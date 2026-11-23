import {convertFileSrc,invoke,isTauri} from '@tauri-apps/api/core';
import type {AppSettings,HomeBackground,HomeBackgroundBuiltinId,HomeBackgroundMediaType,HomeWordmarkStyle} from './settingsModel';

export const builtinBackgrounds:readonly {id:HomeBackgroundBuiltinId;name:string;url:string}[]=[
 {id:'default',name:'默认背景',url:'/home-backgrounds/sunset-condensed-matter-study.png'},
 {id:'style1',name:'样式 1',url:'/home-appearance/background-style-1.png'},
 {id:'style2',name:'样式 2',url:'/home-appearance/background-style-2.png'},
];
export const builtinWordmarks:readonly {id:HomeWordmarkStyle;name:string;url:string}[]=[
 {id:'default',name:'默认 SciDesk',url:'/branding/scidesk-default-transparent.png'},
 {id:'style12',name:'科技蓝 SciDesk',url:'/branding/scidesk-style12-transparent.png'},
];
export const homeBackgrounds:readonly {id:HomeBackground;name:string;url:string}[]=[
 {id:'physics-1',name:'量子蓝图',url:'/home-backgrounds/home-physics-1.png'},{id:'physics-2',name:'分子实验室',url:'/home-backgrounds/home-physics-2.png'},{id:'physics-3',name:'凝聚态晨光',url:'/home-backgrounds/home-physics-3.png'},
];
export const defaultHomeBackground=homeBackgrounds[2];
export const builtinHomeBackground=(id:HomeBackground)=>homeBackgrounds.find(item=>item.id===id)?.url??defaultHomeBackground.url;
export const builtinAppearanceBackground=(id:HomeBackgroundBuiltinId)=>builtinBackgrounds.find(item=>item.id===id)?.url??builtinBackgrounds[0].url;
export const builtinWordmark=(id:HomeWordmarkStyle)=>builtinWordmarks.find(item=>item.id===id)?.url??builtinWordmarks[0].url;
export const pickHomeBackground=()=>isTauri()?invoke<string|null>('pick_home_background'):Promise.resolve(null);
export const copyHomeBackground=(source:string)=>isTauri()?invoke<string>('copy_home_background',{source}):Promise.reject(new Error('请在 SciDesk 桌面应用中选择自定义背景'));
export const readHomeBackground=(path:string)=>isTauri()?invoke<string>('read_home_background',{path}):Promise.reject(new Error('自定义背景不可用，已恢复默认背景。'));
export interface HomeAssetInfo{path:string;media_type:Exclude<HomeBackgroundMediaType,'builtin-image'>;mime:string;data_url:string|null}
export const inspectHomeBackground=(path:string)=>isTauri()?invoke<HomeAssetInfo>('inspect_home_background',{path}):Promise.reject(new Error('自定义背景不可用，已恢复默认背景。'));
export async function resolveHomeBackground(settings:Pick<AppSettings,'home.background_media_type'|'home.background_source'|'home.background_builtin_id'>){
 const mediaType=settings['home.background_media_type'];if(mediaType==='builtin-image')return {type:mediaType,url:builtinAppearanceBackground(settings['home.background_builtin_id']),unavailable:false};
 if(!settings['home.background_source'])return {type:'builtin-image' as const,url:builtinAppearanceBackground('default'),unavailable:true};
 try{const info=await inspectHomeBackground(settings['home.background_source']);return {type:info.media_type,url:info.media_type==='custom-video'?convertFileSrc(info.path):info.data_url!,mime:info.mime,unavailable:false}}catch{return {type:'builtin-image' as const,url:builtinAppearanceBackground('default'),unavailable:true}}
}
export const appearancePresets={
 default:{background:'default' as const,wordmark:'default' as const,subtitleColor:'#ffffff'},
 style1:{background:'style1' as const,wordmark:'style12' as const,subtitleColor:'#07152f'},
 style2:{background:'style2' as const,wordmark:'style12' as const,subtitleColor:'#07152f'},
};
