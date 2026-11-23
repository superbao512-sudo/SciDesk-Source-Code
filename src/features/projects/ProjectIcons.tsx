import {Folder} from 'lucide-react';
import {projectIcons} from './projectIconCatalog';
export function ProjectIcon({name,color,size=24}:{name:string;color:string;size?:number}){const Icon=projectIcons.find(([id])=>id===name)?.[2]??Folder;return <Icon size={size} color={color} strokeWidth={1.8} aria-hidden="true"/>}
