import {invoke,isTauri} from '@tauri-apps/api/core';
export type CaptureRect={x:number;y:number;width:number;height:number};
export type CaptureFrame={rect:CaptureRect;image:string;monitors:CaptureRect[];windows:{id:number;title:string;rect:CaptureRect}[]};
export type CaptureSettings={enabled:boolean;shortcut:string;delay:0|3|5};
export function screenshotCommand<T=unknown>(op:string,args:object={}):Promise<T>{
 if(!isTauri())return Promise.reject(new Error('高级截图需要 SciDesk Windows 桌面端，请在桌面软件中使用'));
 return invoke<T>('screenshot_command',{op,args});
}
export const startScreenshot=(delay=0)=>screenshotCommand('start',{delay});
