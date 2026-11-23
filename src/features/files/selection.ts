export function selectFiles(order:string[],selected:string[],id:string,anchor:string|null,ctrl:boolean,shift:boolean){
 if(shift&&anchor&&order.includes(anchor)){const a=order.indexOf(anchor),b=order.indexOf(id);const range=order.slice(Math.min(a,b),Math.max(a,b)+1);return ctrl?[...new Set([...selected,...range])]:range;}
 return ctrl?selected.includes(id)?selected.filter(k=>k!==id):[...selected,id]:[id];
}
export const physicalDropPoint=(x:number,y:number,scale:number)=>({x:x/(scale||1),y:y/(scale||1)});
