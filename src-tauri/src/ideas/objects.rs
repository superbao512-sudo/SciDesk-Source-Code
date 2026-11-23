//! Shared logical entities retain independent field versions in idea_records.
use super::store::*;
use serde_json::{json,Value};
use sqlx::{Sqlite,Transaction};
use std::collections::BTreeMap;

pub fn initial(kind:&str,id:&str,stamp:u64)->Value {
    let mut record=match kind {
        "room"=>json!({"name":"新包间","icon":"folder","color":"#2563eb","sort_order":0,"canvas_x":60,"canvas_y":60,"width":260,"height":150,"z_index":0}),
        "edge"=>json!({"source_idea_id":"","target_idea_id":"","source_anchor":"right","target_anchor":"left","line_style":"solid","color":"#8199ba","relation_type":"","label":"","directed":false}),
        _=>json!({"type":"text","title":"","content":"","url":"","project_id":null,"room_id":null,"tags":[],"canvas_x":60,"canvas_y":60,"width":300,"height":220,"z_index":0})
    };
    for (k,v) in json!({"id":id,"_entity":kind,"version":0,"created_at":stamp,"updated_at":stamp,"deleted_at":null}).as_object().unwrap(){record[k]=v.clone();}record
}
pub fn validate(kind:&str,patch:&Value)->Result<()> {
    if !["idea","room","edge"].contains(&kind){return Err(error("invalid","对象类型无效"));}
    for (key,value) in patch.as_object().ok_or_else(||error("invalid","修改内容必须为对象"))? {
        let valid=match key.as_str() {
            "deleted"=>value.is_boolean(),
            "canvas_x"|"canvas_y" if kind!="edge"=>value.as_f64().is_some_and(|n|n.is_finite()&&n.abs()<=1_000_000.0),
            "width" if kind!="edge"=>value.as_f64().is_some_and(|n|(180.0..=1600.0).contains(&n)),
            "height" if kind!="edge"=>value.as_f64().is_some_and(|n|(120.0..=1800.0).contains(&n)),
            "z_index"|"sort_order" if kind=="room"||key=="z_index"&&kind=="idea"=>value.as_i64().is_some_and(|n|(0..=1_000_000).contains(&n)),
            "name" if kind=="room"=>value.as_str().is_some_and(|s|!s.trim().is_empty()&&s.chars().count()<=120&&!s.chars().any(char::is_control)),
            "icon" if kind=="room"=>value.as_str().is_some_and(|s|["folder","lightbulb","book","flask","star"].contains(&s)),
            "color" if kind=="room"||kind=="edge"=>value.as_str().is_some_and(|s|s.len()==7&&s.starts_with('#')&&s[1..].bytes().all(|b|b.is_ascii_hexdigit())),
            "source_anchor"|"target_anchor" if kind=="edge"=>value.as_str().is_some_and(|s|["top","right","bottom","left"].contains(&s)),
            "line_style" if kind=="edge"=>value.as_str().is_some_and(|s|["solid","dashed","dashdot","dotted"].contains(&s)),
            "source_idea_id"|"target_idea_id" if kind=="edge"=>value.as_str().is_some_and(valid_id),
            "relation_type" if kind=="edge"=>value.as_str().is_some_and(|s|["","扩展","支持","质疑","引用","验证","相关"].contains(&s)),
            "label" if kind=="edge"=>value.as_str().is_some_and(|s|s.chars().count()<=1000),
            "directed" if kind=="edge"=>value==false,
            "room_id" if kind=="idea"=>value.is_null()||value.as_str().is_some_and(valid_id),
            _ if kind=="idea"=>{let mut one=serde_json::Map::new();one.insert(key.clone(),value.clone());super::store::Store::validate(&Value::Object(one)).is_ok()},
            _=>false
        };if !valid{return Err(error("invalid",format!("字段 {key} 不允许或无效")));}
    }Ok(())
}
async fn target(tx:&mut Transaction<'_,Sqlite>,id:&str,kind:&str)->Result<Value>{
    let text:Option<String>=sqlx::query_scalar("SELECT data FROM idea_records WHERE id=? AND kind=?").bind(id).bind(kind).fetch_optional(&mut **tx).await.map_err(db_error)?;
    let r:Value=serde_json::from_str(&text.ok_or_else(||error("invalid","关联对象不存在"))?).map_err(db_error)?;
    if !r["deleted_at"].is_null(){return Err(error("invalid","关联对象已删除，请先恢复"));}Ok(r)
}
pub async fn validate_links(tx:&mut Transaction<'_,Sqlite>,record:&mut Value)->Result<()> {
    if !record["deleted_at"].is_null(){return Ok(());}
    match record["_entity"].as_str().unwrap_or("idea") {
        "idea"=>{if let Some(room)=record["room_id"].as_str(){target(tx,room,"room").await?;}},
        "edge"=>{
            let a=string(record,"source_idea_id")?;let b=string(record,"target_idea_id")?;
            if a==b{return Err(error("invalid","不能连接到自身"));}target(tx,&a,"idea").await?;target(tx,&b,"idea").await?;
            let (a,b)=if a<b{(a,b)}else{let anchor=record["source_anchor"].clone();record["source_anchor"]=record["target_anchor"].clone();record["target_anchor"]=anchor;(b,a)};record["source_idea_id"]=json!(a);record["target_idea_id"]=json!(b);
            let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM idea_records WHERE kind='edge' AND id<>? AND json_extract(data,'$.deleted_at') IS NULL AND json_extract(data,'$.source_idea_id')=? AND json_extract(data,'$.target_idea_id')=?)").bind(record["id"].as_str()).bind(&a).bind(&b).fetch_one(&mut **tx).await.map_err(db_error)?;
            if duplicate{return Err(error("duplicate","这两个想法已经有关联，请编辑原连接"));}
        },_=>()
    }Ok(())
}
pub async fn write_record(tx:&mut Transaction<'_,Sqlite>,record:&Value,fields:&BTreeMap<String,i64>)->Result<()> {
    sqlx::query("INSERT INTO idea_records(id,data,version,field_versions,kind) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data,version=excluded.version,field_versions=excluded.field_versions")
        .bind(record["id"].as_str()).bind(record.to_string()).bind(record["version"].as_i64()).bind(serde_json::to_string(fields).map_err(db_error)?).bind(record["_entity"].as_str().unwrap_or("idea")).execute(&mut **tx).await.map_err(db_error)?;
    sqlx::query("INSERT INTO idea_changes(idea_id,version) VALUES(?,?)").bind(record["id"].as_str()).bind(record["version"].as_i64()).execute(&mut **tx).await.map_err(db_error)?;Ok(())
}
pub async fn delete_room(tx:&mut Transaction<'_,Sqlite>,record:&Value,args:&Value)->Result<()> {
    let mode=args["delete_mode"].as_str().unwrap_or("move");if !["move","delete"].contains(&mode){return Err(error("invalid","包间删除方式无效"));}
    let rows:Vec<(String,String)>=sqlx::query_as("SELECT data,field_versions FROM idea_records WHERE kind='idea' AND json_extract(data,'$.room_id')=? AND json_extract(data,'$.deleted_at') IS NULL ORDER BY id").bind(record["id"].as_str()).fetch_all(&mut **tx).await.map_err(db_error)?;
    let mut members=Vec::new();for (data,_) in &rows {let r:Value=serde_json::from_str(data).map_err(db_error)?;members.push(json!({"id":r["id"],"version":r["version"]}));}
    // A stale dialog must never move/delete newly added or edited research content.
    if json!(members)!=args.get("members").cloned().unwrap_or(json!([])){return Err(error("room_changed","包间内容已有修改；请刷新后重新确认删除，未更改任何内容"));}
    for (data,fields) in rows {
        let mut r:Value=serde_json::from_str(&data).map_err(db_error)?;let version=r["version"].as_i64().unwrap()+1;let stamp=now();
        let mut f:BTreeMap<String,i64>=serde_json::from_str(&fields).map_err(db_error)?;
        r["room_id"]=Value::Null;f.insert("room_id".into(),version);
        if mode=="delete"{r["deleted_at"]=json!(stamp);f.insert("deleted".into(),version);}
        r["version"]=json!(version);r["updated_at"]=json!(stamp);write_record(tx,&r,&f).await?;
    }Ok(())
}
