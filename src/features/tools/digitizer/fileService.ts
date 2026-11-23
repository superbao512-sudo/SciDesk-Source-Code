import {isTauri} from '@tauri-apps/api/core';
import {screenshotCommand} from '../screenshot/service';
export async function saveText(contents:string,extension:'csv'|'tsv'|'txt'|'json',name:string){
 if(isTauri())return screenshotCommand<{path:string|null}>('digitizer_save',{contents,extension,name});
 const url=URL.createObjectURL(new Blob([contents],{type:extension==='json'?'application/json':'text/plain;charset=utf-8'}));const link=document.createElement('a');link.href=url;link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);return {path:name};
}
export async function copyText(text:string){if(isTauri())await screenshotCommand('digitizer_copy',{text});else await navigator.clipboard.writeText(text);}
export async function clipboardImage(){
 if(isTauri())return screenshotCommand<string>('digitizer_clipboard');
 const items=await navigator.clipboard.read();for(const item of items){const type=item.types.find(t=>t.startsWith('image/'));if(type)return blobData(await item.getType(type));}throw new Error('剪贴板中没有图片');
}
export function blobData(blob:Blob){return new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(new Error('文件读取失败'));reader.readAsDataURL(blob)});}
export function loadImage(url:string){return new Promise<HTMLImageElement>((resolve,reject)=>{const image=new Image();image.onload=()=>resolve(image);image.onerror=()=>reject(new Error('图像无法解码，请使用 PNG、JPEG、BMP 或 WebP'));image.src=url});}
