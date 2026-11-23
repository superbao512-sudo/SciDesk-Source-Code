export type EntityKind='idea'|'room'|'edge';
export interface Idea {id:string;_entity?:EntityKind;room_id?:string|null;type:'text'|'image'|'link'|'file';title:string;content:string;url:string;tags:string[];project_id:string|null;canvas_x:number;canvas_y:number;width:number;height:number;z_index:number;version:number;created_at:number;updated_at:number;deleted_at:number|null;name?:string;icon?:string;color?:string;sort_order?:number}
export interface Room extends Idea {name:string;icon:string;color:string;sort_order:number}
export interface Edge extends Idea {source_idea_id:string;target_idea_id:string;relation_type:string;label:string;directed:boolean;source_anchor:'top'|'right'|'bottom'|'left';target_anchor:'top'|'right'|'bottom'|'left';line_style:'solid'|'dashed'|'dashdot'|'dotted';color:string}
export interface Attachment {id:string;idea_id:string;name:string;media_type:string;size:number;sha256:string;deleted_at:number|null}
export interface CanvasState {x:number;y:number;zoom:number;version:number}
export interface Pending {id:string;op_id:string;base_version:number;entity?:EntityKind;context?:Record<string,unknown>;patch:Record<string,unknown>;before:Record<string,unknown>;sent:boolean;conflict:null|{rejected?:boolean;code:string;message:string;current:Idea;attempted:Partial<Idea>}}
export interface Action {id:string;op_id:string;base_version:number;entity?:EntityKind;context?:Record<string,unknown>;before:Record<string,unknown>;patch:Record<string,unknown>;history:boolean}
export type Transport=(op:string,args?:Record<string,unknown>)=>Promise<unknown>;
export class IdeasClient {
 constructor(name:string,transport:Transport);
 state:{records:Record<string,Idea>;queue:Pending[];uploads:{id:string;idea_id:string;name:string;error:string}[];attachments:Attachment[];cursor:number;identity:string;projects:{id:string;name:string}[];canvas:CanvasState;canvasDraft:CanvasState|null;canvases:Record<string,CanvasState>;canvasDrafts:Record<string,CanvasState>;receipts:Record<string,number>;inputDrafts:Record<string,Partial<Idea>>;resolvedDrafts:unknown[];limits:{attachment_bytes:number}};
 busy:boolean;online:boolean;error:string;localWrites:number;localError:boolean;pending:number;conflicts:Pending[];status:string;
 open():Promise<this>;persist():Promise<void>;diskRead(key:string):Promise<unknown>;diskWrite(key:string,value:unknown):Promise<void>;subscribe(fn:()=>void):()=>void;all():Idea[];view():Idea[];view(entity:'idea'):Idea[];view(entity:'room'):Room[];view(entity:'edge'):Edge[];record(id:string):Idea|undefined;stageInput(id:string,key:string,value:unknown):Promise<void>;editVersion(id:string,key:string,openedVersion:number):number;
 enqueue(id:string,patch:Record<string,unknown>,options?:{baseVersion?:number;beforeValues?:Record<string,unknown>;history?:boolean;entity?:EntityKind;context?:Record<string,unknown>}):Promise<Action>;upload(ideaId:string,file:File):Promise<string>;stageCanvas(canvas:CanvasState,scope?:string):Promise<void>;
 start():void;stop():void;flush():Promise<void>;sync():Promise<void>;retry():Promise<void>;resolve(id:string,choice:'server'|'local'|'manual',merged?:Record<string,unknown>):Promise<void>;
}
export function uid():string;
export function emptyIdea(id?:string):Idea;
export function field(record:Idea,key:string):unknown;
export function attachmentBlob(result:{base64:string;media_type:string}):Blob;
export function blobBase64(blob:Blob):Promise<string>;
export function matching(record:Idea,query:string,project?:string):boolean;

export function validUrl(value:string):boolean;
export const relationTypes:string[];
export function scopeOf(record:Idea):string;
export function locationName(client:IdeasClient,record:Idea):string;
export function nodeName(record?:Idea):string;
export function roomMembers(client:IdeasClient,id:string):{id:string;version:number}[];
export function edgeActive(client:IdeasClient,edge:Edge):boolean;
