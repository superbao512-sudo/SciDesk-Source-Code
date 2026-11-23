//! Localhost-only development bridge. It is never the phone-facing server.
#[cfg(debug_assertions)]
fn main(){if let Err(error)=tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(run()){eprintln!("想法开发服务启动失败：{error}");std::process::exit(1);}}
#[cfg(not(debug_assertions))]
fn main(){eprintln!("此开发桥接不在发布环境运行");std::process::exit(1);}

#[cfg(debug_assertions)]
async fn run()->Result<(),Box<dyn std::error::Error>>{
    use std::{io::BufRead,net::SocketAddr,sync::Arc};
    use axum::{Router,Json,extract::{State,ConnectInfo,Request,DefaultBodyLimit},http::header,middleware::{self,Next},response::Response,routing::post};
    use serde_json::{Value,json};
    use scidesk_lib::ideas::{server::{Hub,response},store::{Store,error,MAX_ATTACHMENT_BYTES}};
    let mut line=String::new();std::io::stdin().lock().read_line(&mut line)?;let options:Value=serde_json::from_str(&line)?;
    let token=options["control_token"].as_str().filter(|s|s.len()==64).ok_or("缺少本地开发凭证")?.to_owned();
    let app_root=scidesk_lib::project_service::application_root()?;
    let root=options["test_root"].as_str().map(std::path::PathBuf::from).unwrap_or_else(||app_root.join("runtime/browser"));
    if !root.starts_with(app_root.join("runtime/browser"))&&!root.starts_with(app_root.join("development/verification")){return Err("想法开发服务只允许第二版隔离目录".into());}
    std::fs::create_dir_all(root.join("database"))?;
    let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(4).connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(root.join("database/scidesk.db")).create_if_missing(true).foreign_keys(true).busy_timeout(std::time::Duration::from_secs(10))).await?;
    scidesk_lib::project_service::migrate(&pool).await?;
    let store=Store::open(root.join("ideas"),pool).await.map_err(|e|std::io::Error::other(e["message"].to_string()))?;
    let hub=Hub::new(store);
    #[derive(Clone)]struct Dev{hub:Arc<Hub>,token:String,address:String}
    async fn auth(State(s):State<Dev>,ConnectInfo(peer):ConnectInfo<SocketAddr>,req:Request,next:Next)->Response{
        let auth=req.headers().get(header::AUTHORIZATION).and_then(|v|v.to_str().ok());
        let origin=req.headers().get(header::ORIGIN).and_then(|v|v.to_str().ok());
        let host=req.headers().get(header::HOST).and_then(|v|v.to_str().ok());
        if !peer.ip().is_loopback()||auth!=Some(format!("Bearer {}",s.token).as_str())||host!=Some(s.address.as_str())||origin.is_some_and(|s|!["http://127.0.0.1:1421","http://localhost:1421"].contains(&s)){return response(Err(error("unauthorized","开发桥接仅供本机应用访问")));}
        next.run(req).await
    }
    async fn command(State(s):State<Dev>,Json(v):Json<Value>)->Response{response(s.hub.command(v["op"].as_str().unwrap_or(""),v.get("args").cloned().unwrap_or(json!({}))).await)}
    let listener=tokio::net::TcpListener::bind(("127.0.0.1",0)).await?;let address=listener.local_addr()?;
    let state=Dev{hub,token,address:address.to_string()};
    let router=Router::new().route("/command",post(command)).layer(DefaultBodyLimit::max(MAX_ATTACHMENT_BYTES.div_ceil(3)*4+8192)).layer(middleware::from_fn_with_state(state.clone(),auth)).with_state(state);
    println!("{}",json!({"port":address.port()}));
    axum::serve(listener,router.into_make_service_with_connect_info::<SocketAddr>()).await?;Ok(())
}
