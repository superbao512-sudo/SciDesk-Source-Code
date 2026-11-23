import {projectCommand} from '../projects/projectService';
export type Category={id:string;parent_id:string;name:string;sort_order:number};
export type Resource={id:string;title:string;url:string;category1:string;category2:string;property:string;tier:'core'|'optional';origin:'excel'|'manual';source_id:string|null;folder:string;classification_note:string;duplicate:string;notes:string;tags:string;created_at:string;updated_at:string;removed:boolean};
export type Backup={id:string;resource_id:string;path:string;available:boolean};
export type ImportReport={source:string;sha256:string;total:number;core:number;optional:number;excluded:number;duplicates:number;eligible_duplicates:number;invalid_urls:number;unknown_categories:number;created?:number;skipped?:number;exceptions:{source_id:string;title:string;url:string;issue:string;raw:Record<string,unknown>}[]};
export type LearningState={resources:Resource[];categories:Category[];backups:Backup[];report:ImportReport|null};
export const learningCommand=<T,>(op:string,args:Record<string,unknown>={})=>projectCommand<T>(`learning_${op}`,args);

// Workbook category IDs survive renames and reordering. User-created categories
// can share an original name when created before the first workbook import.
const subjects = [
 {name:'材料物理与第一性原理',categories:[
  [0,'第一性原理与电子结构计算'],[1,'声子、热输运与电子-声子耦合'],[5,'拓扑、Wannier 与量子输运'],
  [9,'材料结构与科研数据库'],[14,'科研软件与结构预测工具'],[15,'Quantum ESPRESSO 与第一性原理计算']]},
 {name:'科研计算与数据分析',categories:[
  [3,'科研编程与代码开发'],[7,'Linux、服务器与高性能计算'],[10,'科研软件官方手册与参考文档'],[12,'科研软件与分析工具'],[13,'科研可视化与数据处理']]},
 {name:'课程、讲座与交流',categories:[[2,'在线课程、讲座与物理教学'],[8,'科研社区与第三方教程'],[16,'理论基础与物理课程']]},
 {name:'写作与科研服务',categories:[[4,'学术写作与排版'],[6,'科研辅助工具与资料整理'],[11,'可选：学校、科研机构与个人学术服务']]},
] as const;

export function learningDirectory(categories:Category[],resources:Resource[]){
 const counts=new Map<string,number>();
 for(const row of resources)counts.set(row.category1,(counts.get(row.category1)??0)+1);
 const groups=[...subjects.map(subject=>subject.name as string),'其他与自定义分类'].map(name=>({name,entries:[] as {name:string;count:number}[]}));
 const names=new Set<string>();
 for(const category of categories.filter(c=>!c.parent_id)){
  names.add(category.name);
  const count=counts.get(category.name)??0;
  if(!count)continue;
  const index=subjects.findIndex(group=>group.categories.some(([i,name])=>category.id===`excel:${i}`||category.name===name));
  groups[index<0?groups.length-1:index].entries.push({name:category.name,count});
 }
 for(const [name,count] of counts)if(!names.has(name))groups[groups.length-1].entries.push({name,count});
 return groups.filter(group=>group.entries.length);
}
export function filterResources(rows:Resource[],query:string,category1:string,category2:string,tier:string,property:string){
 const words=query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
 const eligible=rows.filter(r=>(!category1||r.category1===category1)&&(!category2||r.category2===category2)&&(tier==='all'||r.tier===tier)&&(!property||r.property===property));
 if(!words.length)return eligible;
 const matches:{resource:Resource;score:number}[]=[];
 for(const resource of eligible){
  const title=resource.title.toLocaleLowerCase();
  const direct=[resource.tags,resource.url].join(' ').toLocaleLowerCase();
  const context=[resource.category1,resource.category2,resource.property,resource.notes,resource.classification_note].join(' ').toLocaleLowerCase();
  let score=0;
  for(const word of words){
   if(title.includes(word))score+=4;
   else if(direct.includes(word))score+=2;
   else if(context.includes(word))score+=1;
   else{score=0;break;}
  }
  if(score)matches.push({resource,score});
 }
 return matches.sort((a,b)=>b.score-a.score).map(m=>m.resource);
}
