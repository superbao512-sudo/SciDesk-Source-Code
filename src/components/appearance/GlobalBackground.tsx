import type {SyntheticEvent} from 'react';
import type {HomeBackgroundMediaType} from '../../features/settings/settingsModel';

export function GlobalBackground({type,url,mime,capturePreview,mediaError}:{type:HomeBackgroundMediaType;url:string;mime?:string;capturePreview?:(url:string)=>void;mediaError?:()=>void}){
 const snapshot=(event:SyntheticEvent<HTMLImageElement|HTMLVideoElement>)=>{
  if(!capturePreview)return;
  const media=event.currentTarget,width=media instanceof HTMLVideoElement?media.videoWidth:media.naturalWidth,height=media instanceof HTMLVideoElement?media.videoHeight:media.naturalHeight;
  if(!width||!height)return;
  try{const canvas=document.createElement('canvas');canvas.width=Math.min(width,480);canvas.height=Math.round(height*canvas.width/width);canvas.getContext('2d')!.drawImage(media,0,0,canvas.width,canvas.height);capturePreview(canvas.toDataURL('image/png'))}catch{/* Unavailable/tainted frames show a placeholder; never add another player. */}
 };
 return <div className="global-background" aria-hidden="true">{type==='custom-video'
  ?<video key={url} className="global-background-media" src={url} autoPlay muted loop playsInline preload="auto" disablePictureInPicture onLoadedData={snapshot} onError={mediaError}><source src={url} type={mime}/></video>
  :<img className="global-background-media" src={url} alt="" onLoad={snapshot} onError={mediaError}/>}</div>;
}
