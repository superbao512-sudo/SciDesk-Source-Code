import {query} from '../../services/database';
import {settingsDefaults,validateSetting,type AppSettings,type SettingKey} from './settingsModel';
import {applySurfaces} from './appearanceSurface';
export async function getAppSettings():Promise<AppSettings>{
 const rows=await query<{key:string;value:string}[]>('SELECT key,value FROM settings');const config=structuredClone(settingsDefaults);
 for(const {key,value:text} of rows){if(!(key in config))continue;let value:unknown=text;try{value=JSON.parse(text)}catch{/* Legacy strings remain supported. */}try{validateSetting(key as SettingKey,value as never);Object.assign(config,{[key]:value})}catch{/* Use bundled defaults for invalid preferences. */}}return config;
}
export const getAllSettings=getAppSettings;
export function applyAppearance(config:AppSettings){document.documentElement.lang='zh-CN';document.documentElement.dataset.density=config['appearance.density'];document.documentElement.dataset.colorTheme=config['appearance.color_theme'];applySurfaces(config);}
let pending:Promise<void>=Promise.resolve();
export function setSetting<K extends SettingKey>(key:K,value:AppSettings[K]):Promise<void>{
 const write=pending.then(async()=>{validateSetting(key,value);await query('INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',[key,JSON.stringify(value)],false);applyAppearance(await getAppSettings());window.dispatchEvent(new Event('scidesk:settings-changed'));});pending=write.catch(()=>{});return write;
}
