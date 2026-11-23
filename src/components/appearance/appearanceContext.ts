import {createContext,useContext} from 'react';
import {settingsDefaults,type AppSettings,type HomeBackgroundMediaType} from '../../features/settings/settingsModel';
import {builtinAppearanceBackground} from '../../features/settings/homeBackgrounds';

export interface BackgroundState {type:HomeBackgroundMediaType;url:string;mime?:string;unavailable:boolean}
export const defaultBackground:BackgroundState={type:'builtin-image',url:builtinAppearanceBackground('default'),unavailable:false};
interface ApplicationAppearanceValue {config:AppSettings;background:BackgroundState;previewUrl:string;capturePreview:(url:string)=>void;mediaError:()=>void}
export const AppearanceContext=createContext<ApplicationAppearanceValue>({config:settingsDefaults,background:defaultBackground,previewUrl:'',capturePreview:()=>{},mediaError:()=>{}});
export const useApplicationAppearance=()=>useContext(AppearanceContext);
