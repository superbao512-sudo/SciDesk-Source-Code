import type {CSSProperties} from 'react';
import type {HomeCompositionId,HomeWordmarkStyle} from './settingsModel';

export const HOME_REFERENCE={windowWidth:1536,windowHeight:1024,sidebarWidth:230,heroTop:104,heroWidth:1306,heroHeight:920} as const;
export interface HomeCompositionProfile {
 wordmarkWidthRatio:number;brandTopRatio:number;brandCenterRatio:number;subtitleGap:number;
 searchWidthRatio:number;searchCenterRatio:number;searchHeight:number;searchGap:number;searchRadius:number;greetingGap:number;
}
export const HOME_COMPOSITIONS:Record<HomeCompositionId,HomeCompositionProfile>={
 default:{wordmarkWidthRatio:.46,brandTopRatio:.24,brandCenterRatio:.494,subtitleGap:14,searchWidthRatio:.58,searchCenterRatio:.484,searchHeight:74,searchGap:25,searchRadius:24,greetingGap:52},
 style1:{wordmarkWidthRatio:.475,brandTopRatio:.195,brandCenterRatio:.479,subtitleGap:14,searchWidthRatio:.63,searchCenterRatio:.480,searchHeight:78,searchGap:27,searchRadius:26,greetingGap:52},
 style2:{wordmarkWidthRatio:.44,brandTopRatio:.26,brandCenterRatio:.483,subtitleGap:14,searchWidthRatio:.59,searchCenterRatio:.510,searchHeight:75,searchGap:25,searchRadius:24,greetingGap:48},
};
// Dimensions of the deterministic runtime crops. Optical bounds (alpha >= 16)
// align the visible artwork without deleting faint pixels outside these bounds.
// All original RGBA values, including the faint perimeter, remain in the image.
export const WORDMARK_GEOMETRY={
 default:{width:2045,height:698,optical:[4,98,2038,596]},
 style12:{width:1057,height:254,optical:[9,4,1051,247]},
} as const;

export function homeCompositionStyle(id:HomeCompositionId,wordmark:HomeWordmarkStyle,width:number,height:number,subtitleSize:number,subtitleVisible:boolean,subtitleHeight?:number):CSSProperties {
 const profile=HOME_COMPOSITIONS[id],geometry=WORDMARK_GEOMETRY[wordmark];
 const scale=Math.min(width/HOME_REFERENCE.heroWidth,height/HOME_REFERENCE.heroHeight);
 const [left,top,right,bottom]=geometry.optical;
 const visibleWidth=HOME_REFERENCE.heroWidth*profile.wordmarkWidthRatio*scale;
 const imageScale=visibleWidth/(right-left),imageWidth=geometry.width*imageScale;
 const brandTop=height*profile.brandTopRatio;
 const subtitleTop=brandTop+(bottom-top)*imageScale+profile.subtitleGap*scale;
 const searchTop=subtitleVisible?subtitleTop+(subtitleHeight??subtitleSize*1.15*scale)+profile.searchGap*scale:brandTop+(bottom-top)*imageScale+profile.searchGap*scale;
 const px=(value:number)=>`${value}px`;
 return {
  '--home-scale':scale,'--wordmark-width':px(imageWidth),'--wordmark-left':px(width*profile.brandCenterRatio-visibleWidth/2-left*imageScale),
  '--brand-center':px(width*profile.brandCenterRatio),'--search-center':px(width*profile.searchCenterRatio),
  '--brand-top':px(brandTop-top*imageScale),'--subtitle-top':px(subtitleTop),'--subtitle-size':px(subtitleSize*scale),
  '--search-top':px(searchTop),'--search-width':px(HOME_REFERENCE.heroWidth*profile.searchWidthRatio*scale),
  '--search-height':px(profile.searchHeight*scale),'--search-radius':px(profile.searchRadius*scale),
  '--greeting-top':px(searchTop+(profile.searchHeight+profile.greetingGap)*scale),
 } as CSSProperties;
}
