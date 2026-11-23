import {isTauri} from '@tauri-apps/api/core';
import Database from '@tauri-apps/plugin-sql';
export async function query<T>(sql:string,values:unknown[]=[],select=true):Promise<T>{
 if(isTauri()){const db=Database.get('sqlite:scidesk.db');return (select?db.select<T>(sql,values):db.execute(sql,values)) as Promise<T>;}
 if(!import.meta.env.DEV)throw new Error('请使用 SciDesk 桌面应用或 npm run dev。');
 const response=await fetch('/__scidesk/db',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({sql,values,select})});
 const data=await response.json();if(!response.ok)throw new Error(data.error||'数据库访问失败');return data as T;
}
