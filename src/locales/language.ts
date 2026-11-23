export function uiText(text:string,...values:unknown[]):string{
 return text.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<values.length?String(values[Number(i)]):match);
}
