use std::{collections::HashMap, net::{IpAddr,Ipv4Addr,SocketAddr}, sync::Arc, time::Duration};
use axum::{Router,Json,body::Body,extract::{State,ConnectInfo,Request,Path,DefaultBodyLimit},http::{HeaderMap,StatusCode,header},middleware::{self,Next},response::{IntoResponse,Response},routing::{get,post}};
use serde::Serialize;
use serde_json::{json,Value};
use tokio::{net::TcpListener,sync::{Mutex,oneshot,Semaphore}};
use super::store::*;

#[derive(Clone,Serialize)]
pub struct Network { pub name:String,pub ip:Ipv4Addr,pub netmask:Ipv4Addr }
pub fn networks()->Result<Vec<Network>> {
    let mut result=Vec::new();
    for interface in if_addrs::get_if_addrs().map_err(|_|error("network","无法读取网络适配器"))? {
        if let if_addrs::IfAddr::V4(v4)=interface.addr {if v4.ip.is_private()&&!v4.ip.is_loopback()&&!result.iter().any(|n:&Network|n.ip==v4.ip){result.push(Network{name:interface.name,ip:v4.ip,netmask:v4.netmask});}}
    }Ok(result)
}
struct Ticket { token:String,expires:u64 }
struct Pending { id:String,name:String,poll_hash:String,expires:u64,rejected:bool,approved:Option<(String,String)> }
struct Active { address:SocketAddr,network:Network,stop:oneshot::Sender<()>,ticket:Option<Ticket> }
#[derive(Default)]
struct Control { active:Option<Active>, pending:Vec<Pending> }
pub struct Hub { pub store:Arc<Store>,control:Mutex<Control> }
#[derive(Clone)]
struct Lan { hub:Arc<Hub>,network:Network,address:SocketAddr,test_loopback:bool,rate:Arc<Mutex<HashMap<IpAddr,(u64,u32)>>>,slots:Arc<Semaphore> }
pub fn response(result:Result<Value>)->Response {
    match result {Ok(value)=>(StatusCode::OK,Json(json!({"ok":true,"value":value}))).into_response(),Err(e)=>{let status=match e["code"].as_str(){Some("unauthorized"|"revoked")=>StatusCode::UNAUTHORIZED,Some("conflict")=>StatusCode::CONFLICT,Some("rate")=>StatusCode::TOO_MANY_REQUESTS,Some("disconnected")=>StatusCode::FORBIDDEN,_=>StatusCode::BAD_REQUEST};(status,Json(json!({"ok":false,"error":e}))).into_response()}}
}
impl Hub {
    pub fn new(store:Store)->Arc<Self>{Arc::new(Self{store:Arc::new(store),control:Mutex::new(Control::default())})}
    pub async fn data(&self,op:&str,args:Value)->Result<Value>{
        match op {"snapshot"=>self.store.snapshot(args["cursor"].as_i64()).await,"mutate"=>self.store.mutate(args).await,"upload"=>self.store.upload(args).await,"attachment"=>self.store.attachment(&string(&args,"id")?).await,"remove_attachment"=>self.store.remove_attachment(&string(&args,"id")?).await,"canvas"=>self.store.canvas(args).await,"projects"=>self.store.projects().await,_=>Err(error("invalid","不支持的想法操作"))}
    }
    pub async fn command(self:&Arc<Self>,op:&str,args:Value)->Result<Value>{
        match op {
            "interfaces"=>Ok(json!(networks()?)),"connection_status"=>self.status().await,
            "start"=>{let ip:Ipv4Addr=string(&args,"ip")?.parse().map_err(|_|error("network","连接地址无效"))?;let network=networks()?.into_iter().find(|n|n.ip==ip).ok_or_else(||error("network","所选局域网地址已不可用，请重新检测"))?;self.start(network,false).await?;self.pair().await},
            "pair"=>self.pair().await,
            "pair_link"=>{let c=self.control.lock().await;let active=c.active.as_ref().ok_or_else(||error("network","手机连接服务尚未启动"))?;let ticket=active.ticket.as_ref().filter(|t|t.expires>now()).ok_or_else(||error("expired","请重新生成配对码"))?;Ok(json!(format!("http://{}/#pair={}",active.address,ticket.token)))},
            "stop"=>{let mut c=self.control.lock().await;if let Some(active)=c.active.take(){let _=active.stop.send(());}c.pending.clear();Ok(json!(true))},
            "approve"|"reject"=>self.approve(&string(&args,"id")?,op=="approve").await,
            "revoke"|"disconnect"|"resume"=>{let id=string(&args,"id")?;let column=if op=="revoke"{"revoked_at"}else{"disconnected_at"};let value=if op=="resume"{None}else{Some(now() as i64)};sqlx::query(&format!("UPDATE idea_devices SET {column}=? WHERE id=?")).bind(value).bind(id).execute(&self.store.pool).await.map_err(db_error)?;Ok(json!(true))},
            "open_url"=>{let target=string(&args,"url")?;if !safe_url(&target){return Err(error("invalid","只允许明确打开有效的 HTTP 或 HTTPS 链接"));}crate::project_platform::open(std::path::Path::new(&target)).map_err(|_|error("external","系统无法打开此链接"))?;Ok(json!(true))},
            _=>self.data(op,args).await
        }
    }
    pub async fn start(self:&Arc<Self>,network:Network,test_loopback:bool)->Result<SocketAddr>{
        if !(network.ip.is_private()||test_loopback&&network.ip.is_loopback()){return Err(error("network","只能绑定选定的私有局域网地址"));}
        let mut control=self.control.lock().await;
        if let Some(active)=&control.active {if active.network.ip==network.ip{return Ok(active.address);}return Err(error("network","请先停止当前连接，再切换网卡"));}
        let preferred:Option<String>=sqlx::query_scalar("SELECT value FROM idea_settings WHERE key='network_port'").fetch_optional(&self.store.pool).await.map_err(db_error)?;
        let port=if test_loopback{0}else{preferred.and_then(|s|serde_json::from_str::<u16>(&s).ok()).unwrap_or(0)};
        let listener=match TcpListener::bind((network.ip,port)).await {Ok(l)=>l,Err(_) if port!=0=>TcpListener::bind((network.ip,0)).await.map_err(|_|error("network","无法绑定此局域网地址；请检查网络后重试"))?,Err(_)=>return Err(error("network","无法启动局域网服务"))};
        let address=listener.local_addr().map_err(db_error)?;let (stop,rx)=oneshot::channel();
        let lan=Lan{hub:self.clone(),network:network.clone(),address,test_loopback,rate:Arc::new(Mutex::new(HashMap::new())),slots:Arc::new(Semaphore::new(8))};
        let router=Router::new().route("/",get(index)).route("/mobile.css",get(css)).route("/mobile.js",get(script)).route("/core.js",get(core))
            .route("/pair/request",post(pair_request).layer(DefaultBodyLimit::max(4096))).route("/pair/claim",post(pair_claim).layer(DefaultBodyLimit::max(4096)))
            .route("/api/{op}",post(api)).layer(DefaultBodyLimit::max(MAX_ATTACHMENT_BYTES.div_ceil(3)*4+8192)).layer(middleware::from_fn_with_state(lan.clone(),guard)).with_state(lan);
        tokio::spawn(async move {let _=axum::serve(listener,router.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async{let _=rx.await;}).await;});
        control.active=Some(Active{address,network,stop,ticket:None});
        sqlx::query("INSERT INTO idea_settings(key,value) VALUES('network_port',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").bind(address.port().to_string()).execute(&self.store.pool).await.map_err(db_error)?;
        Ok(address)
    }
    async fn pair(&self)->Result<Value>{let mut c=self.control.lock().await;let active=c.active.as_mut().ok_or_else(||error("network","手机连接服务尚未启动"))?;active.ticket=Some(Ticket{token:secret(),expires:now()+120_000});drop(c);self.status().await}
    pub async fn status(&self)->Result<Value>{
        let mut c=self.control.lock().await;c.pending.retain(|p|p.expires>now());
        let (address,qr,expires,available)=if let Some(a)=&c.active {
            let present=networks()?.iter().any(|n|n.ip==a.network.ip)||a.network.ip.is_loopback();
            if let Some(t)=&a.ticket {let url=format!("http://{}/#pair={}",a.address,t.token);let svg=qrcode::QrCode::new(url.as_bytes()).map_err(db_error)?.render::<qrcode::render::svg::Color>().min_dimensions(240,240).build();(format!("http://{}/",a.address),Some(svg),Some(t.expires),present)}else{(format!("http://{}/",a.address),None,None,present)}
        }else{(String::new(),None,None,false)};
        let pending:Vec<Value>=c.pending.iter().filter(|p|!p.rejected&&p.approved.is_none()).map(|p|json!({"id":p.id,"name":p.name,"expires":p.expires})).collect();drop(c);
        let rows=sqlx::query("SELECT id,name,created_at,last_seen,revoked_at,disconnected_at FROM idea_devices ORDER BY created_at DESC").fetch_all(&self.store.pool).await.map_err(db_error)?;
        use sqlx::Row;
        let devices:Vec<Value>=rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"created_at":r.get::<i64,_>("created_at"),"last_seen":r.get::<i64,_>("last_seen"),"revoked_at":r.get::<Option<i64>,_>("revoked_at"),"disconnected_at":r.get::<Option<i64>,_>("disconnected_at")})).collect();
        Ok(json!({"running":!address.is_empty(),"address":address,"qr_svg":qr,"expires":expires,"network_available":available,"pending":pending,"devices":devices}))
    }
    pub async fn request_pair(&self,args:Value)->Result<Value>{
        let token=string(&args,"token")?;let name=string(&args,"name")?;if name.trim().is_empty()||name.chars().count()>80||name.chars().any(char::is_control){return Err(error("invalid","设备名称需要 1–80 个字符"));}
        let mut c=self.control.lock().await;let active=c.active.as_mut().ok_or_else(||error("network","连接已断开"))?;
        let t=active.ticket.as_ref().ok_or_else(||error("expired","配对码已使用，请在电脑上重新生成"))?;
        if t.expires<=now()||hash(token.as_bytes())!=hash(t.token.as_bytes()){return Err(error("expired","配对码无效或已过期，请重新扫码"));}
        active.ticket=None;let poll=secret();let id=random_id();c.pending.push(Pending{id:id.clone(),name:name.trim().into(),poll_hash:hash(poll.as_bytes()),expires:now()+120_000,rejected:false,approved:None});
        Ok(json!({"id":id,"poll_token":poll,"expires":now()+120_000,"state":"waiting_confirmation"}))
    }
    async fn approve(&self,id:&str,allow:bool)->Result<Value>{
        let mut c=self.control.lock().await;let p=c.pending.iter_mut().find(|p|p.id==id&&p.expires>now()&&!p.rejected&&p.approved.is_none()).ok_or_else(||error("expired","配对请求已失效"))?;
        if !allow{p.rejected=true;return Ok(json!(true));}
        let device=random_id();let token=secret();let stamp=now() as i64;
        sqlx::query("INSERT INTO idea_devices(id,name,token_hash,created_at,last_seen) VALUES(?,?,?,?,?)").bind(&device).bind(&p.name).bind(hash(token.as_bytes())).bind(stamp).bind(stamp).execute(&self.store.pool).await.map_err(db_error)?;
        p.approved=Some((device,token));Ok(json!(true))
    }
    pub async fn claim(&self,args:Value)->Result<Value>{
        let id=string(&args,"id")?;let poll=string(&args,"poll_token")?;let c=self.control.lock().await;
        let p=c.pending.iter().find(|p|p.id==id&&p.poll_hash==hash(poll.as_bytes())&&p.expires>now()).ok_or_else(||error("expired","配对请求已过期，请重新扫码"))?;
        if p.rejected{return Err(error("unauthorized","电脑端已拒绝本次配对"));}
        if let Some((device,token))=&p.approved{Ok(json!({"state":"connected","device_id":device,"token":token,"name":p.name}))}else{Ok(json!({"state":"waiting_confirmation"}))}
    }
    pub async fn authenticate(&self,token:&str)->Result<()> {
        if token.len()!=64{return Err(error("unauthorized","设备未授权，请重新扫码配对"));}
        let device:Option<(String,Option<i64>,Option<i64>)>=sqlx::query_as("SELECT id,revoked_at,disconnected_at FROM idea_devices WHERE token_hash=?").bind(hash(token.as_bytes())).fetch_optional(&self.store.pool).await.map_err(db_error)?;
        let (id,revoked,disconnected)=device.ok_or_else(||error("unauthorized","设备凭证无效，请重新扫码"))?;
        if revoked.is_some(){return Err(error("revoked","电脑已撤销此设备；未同步草稿仍保留在手机"));}
        if disconnected.is_some(){return Err(error("disconnected","此设备已断开，请在电脑上允许重连；草稿已保留"));}
        sqlx::query("UPDATE idea_devices SET last_seen=? WHERE id=?").bind(now() as i64).bind(id).execute(&self.store.pool).await.map_err(db_error)?;Ok(())
    }
}
fn bearer(headers:&HeaderMap)->&str{headers.get(header::AUTHORIZATION).and_then(|v|v.to_str().ok()).and_then(|s|s.strip_prefix("Bearer ")).unwrap_or("")}
async fn guard(State(lan):State<Lan>,ConnectInfo(peer):ConnectInfo<SocketAddr>,req:Request,next:Next)->Response {
    let allowed=match peer.ip(){IpAddr::V4(ip)=>(ip.is_private()&&u32::from(ip)&u32::from(lan.network.netmask)==u32::from(lan.network.ip)&u32::from(lan.network.netmask))||lan.test_loopback&&ip.is_loopback(),_=>false};
    let expected=lan.address.to_string();let host=req.headers().get(header::HOST).and_then(|v|v.to_str().ok());
    let origin=req.headers().get(header::ORIGIN).and_then(|v|v.to_str().ok());
    if !allowed||host!=Some(expected.as_str())||origin.is_some_and(|s|s!=format!("http://{expected}")){return response(Err(error("unauthorized","此连接不属于允许的局域网来源")));}
    let stamp=now()/60_000;let mut rates=lan.rate.lock().await;rates.retain(|_,(minute,_)|*minute==stamp);let item=rates.entry(peer.ip()).or_insert((stamp,0));item.1+=1;if item.1>300{ return response(Err(error("rate","请求过于频繁，请稍后重试")));}drop(rates);
    let Ok(_permit)=lan.slots.clone().try_acquire_owned()else{return response(Err(error("rate","连接繁忙，请稍后重试")));};
    if req.uri().path().starts_with("/api/"){if let Err(e)=lan.hub.authenticate(bearer(req.headers())).await{return response(Err(e));}}
    let mut output=match tokio::time::timeout(Duration::from_secs(25),next.run(req)).await{Ok(r)=>r,Err(_)=>response(Err(error("timeout","请求超时，草稿可稍后重试")))};
    let h=output.headers_mut();h.insert(header::CACHE_CONTROL,"no-store".parse().unwrap());h.insert("X-Content-Type-Options","nosniff".parse().unwrap());h.insert("Referrer-Policy","no-referrer".parse().unwrap());h.insert("Content-Security-Policy","default-src 'self'; img-src 'self' blob: data:; style-src 'self'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'".parse().unwrap());output
}
async fn api(State(lan):State<Lan>,Path(op):Path<String>,Json(args):Json<Value>)->Response{
    if !["snapshot","mutate","upload","attachment","remove_attachment","projects"].contains(&op.as_str()){return response(Err(error("unauthorized","此接口不对手机开放")));}
    response(lan.hub.data(&op,args).await)
}
async fn pair_request(State(lan):State<Lan>,Json(args):Json<Value>)->Response{response(lan.hub.request_pair(args).await)}
async fn pair_claim(State(lan):State<Lan>,Json(args):Json<Value>)->Response{response(lan.hub.claim(args).await)}
fn asset(name:&str,embedded:&'static str,kind:&'static str)->Response{
    // Fixed call-site names only. Production assets are embedded in the desktop binary.
    #[cfg(debug_assertions)]
    let content=std::fs::read_to_string(std::path::Path::new(env!("SCIDESK_PROJECT_ROOT")).join("public/ideas-mobile").join(name)).unwrap_or_else(|_|embedded.to_owned());
    #[cfg(not(debug_assertions))]
    let content={let _=name;embedded.to_owned()};
    ([(header::CONTENT_TYPE,kind)],Body::from(content)).into_response()
}
async fn index()->Response{asset("index.html",include_str!("../../../public/ideas-mobile/index.html"),"text/html; charset=utf-8")}
async fn css()->Response{asset("mobile.css",include_str!("../../../public/ideas-mobile/mobile.css"),"text/css; charset=utf-8")}
async fn script()->Response{asset("mobile.js",include_str!("../../../public/ideas-mobile/mobile.js"),"text/javascript; charset=utf-8")}
async fn core()->Response{asset("core.js",include_str!("../../../public/ideas-mobile/core.js"),"text/javascript; charset=utf-8")}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    async fn fixture()->Arc<Hub>{
        let root=std::path::PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/ideas-network-tests").join(random_id());
        let projects=sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql("CREATE TABLE projects(id TEXT,name TEXT,deleted_at TEXT,sort_order INTEGER)").execute(&projects).await.unwrap();
        Hub::new(Store::open(root,projects).await.unwrap())
    }
    async fn http(address:SocketAddr,path:&str,args:Value,token:Option<&str>,origin:Option<&str>)->(u16,Value){
        let mut socket=tokio::net::TcpStream::connect(address).await.unwrap();let body=args.to_string();
        let request=format!("POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nConnection: close\r\n{}{}Content-Length: {}\r\n\r\n{body}",token.map(|s|format!("Authorization: Bearer {s}\r\n")).unwrap_or_default(),origin.map(|s|format!("Origin: {s}\r\n")).unwrap_or_default(),body.len());
        socket.write_all(request.as_bytes()).await.unwrap();let mut bytes=Vec::new();socket.read_to_end(&mut bytes).await.unwrap();let text=String::from_utf8(bytes).unwrap();let (head,body)=text.split_once("\r\n\r\n").unwrap();(head.split_whitespace().nth(1).unwrap().parse().unwrap(),serde_json::from_str(body).unwrap())
    }
    async fn connection(hub:&Arc<Hub>)->(SocketAddr,String,String){
        let address=hub.start(Network{name:"test-loopback".into(),ip:Ipv4Addr::LOCALHOST,netmask:Ipv4Addr::new(255,0,0,0)},true).await.unwrap();hub.pair().await.unwrap();
        let ticket=hub.control.lock().await.active.as_ref().unwrap().ticket.as_ref().unwrap().token.clone();
        let (_,request)=http(address,"/pair/request",json!({"token":ticket,"name":"模拟手机"}),None,None).await;let pending=request["value"].clone();
        assert_eq!(http(address,"/pair/claim",pending.clone(),None,None).await.1["value"]["state"],"waiting_confirmation");
        assert!(hub.request_pair(json!({"token":ticket,"name":"重复扫码"})).await.is_err());
        hub.command("approve",json!({"id":pending["id"]})).await.unwrap();let (_,claim)=http(address,"/pair/claim",pending,None,None).await;
        (address,claim["value"]["token"].as_str().unwrap().into(),claim["value"]["device_id"].as_str().unwrap().into())
    }
    #[tokio::test]
    async fn actual_http_pair_confirmation_sync_and_revocation(){
        let hub=fixture().await;let (address,token,device)=connection(&hub).await;
        assert_eq!(http(address,"/api/snapshot",json!({}),None,None).await.0,401);
        assert_eq!(http(address,"/api/snapshot",json!({}),Some("bad"),None).await.0,401);
        assert_eq!(http(address,"/api/open_url",json!({}),Some(&token),None).await.0,401);
        assert_eq!(http(address,"/api/snapshot",json!({}),Some(&token),Some("http://evil.invalid")).await.0,401);
        let id=random_id();let op=json!({"id":id,"op_id":random_id(),"base_version":0,"patch":{"title":"模拟手机创建","content":"离线草稿上传"}});
        let (code,result)=http(address,"/api/mutate",op.clone(),Some(&token),None).await;assert_eq!(code,200);assert_eq!(http(address,"/api/mutate",op,Some(&token),None).await.1,result);
        assert_eq!(hub.store.record(&id).await.unwrap()["content"],"离线草稿上传");
        hub.data("mutate",json!({"id":id,"op_id":random_id(),"base_version":1,"patch":{"canvas_x":321}})).await.unwrap();
        assert_eq!(http(address,"/api/snapshot",json!({}),Some(&token),None).await.1["value"]["records"][0]["canvas_x"],321);
        hub.command("disconnect",json!({"id":device})).await.unwrap();assert_eq!(http(address,"/api/snapshot",json!({}),Some(&token),None).await.0,403);
        hub.command("resume",json!({"id":device})).await.unwrap();assert_eq!(http(address,"/api/snapshot",json!({}),Some(&token),None).await.0,200);
        hub.command("revoke",json!({"id":device})).await.unwrap();assert_eq!(http(address,"/api/snapshot",json!({}),Some(&token),None).await.0,401);
        assert_eq!(hub.store.record(&id).await.unwrap()["title"],"模拟手机创建");hub.command("stop",json!({})).await.unwrap();
    }
    #[tokio::test]
    async fn expiry_qr_local_generation_and_restart_identity(){
        let hub=fixture().await;let (_,token,_)=connection(&hub).await;
        hub.pair().await.unwrap();let qr=hub.status().await.unwrap();assert!(qr["qr_svg"].as_str().unwrap().contains("<svg"));
        let expired={let mut c=hub.control.lock().await;let t=c.active.as_mut().unwrap().ticket.as_mut().unwrap();t.expires=now()-1;t.token.clone()};
        assert_eq!(hub.request_pair(json!({"token":expired,"name":"过期设备"})).await.unwrap_err()["code"],"expired");
        let identity=hub.store.setting("identity").await.unwrap();hub.command("stop",json!({})).await.unwrap();
        let reopened=Hub::new(Store::open(hub.store.root.clone(),hub.store.project_pool.clone()).await.unwrap());assert_eq!(reopened.store.setting("identity").await.unwrap(),identity);assert!(reopened.authenticate(&token).await.is_ok());
    }
}
