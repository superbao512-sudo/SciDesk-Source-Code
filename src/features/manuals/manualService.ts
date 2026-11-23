import {projectCommand} from '../projects/projectService';
export type Manual={id:string;name:string;category:string;description:string;icon_name:string;manual_color:string;sort_order:number;name_font_size:number;links:number;invalid:number};
export type LocalLink={id:string;manual_id:string;path:string;kind:'file'|'folder';status:'available'|'missing'|'unavailable';message:string;scanned_at:string|null};
export type ManualDetail=Omit<Manual,'links'|'invalid'>&{links:LocalLink[]};
export type ManualEntry={relative:string;name:string;is_dir:boolean;size:number|null;modified:number|null;link_id?:string};
export const manualCommand=<T,>(op:string,args:Record<string,unknown>={})=>projectCommand<T>(`manual_${op}`,args);
export const localName=(path:string)=>path.replace(/[\\/]+$/,'').split(/[\\/]/).at(-1)||path;
