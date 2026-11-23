//! Authenticated localhost-only bridge for the existing Vite development workflow.
#[cfg(debug_assertions)]
fn main(){if let Err(e)=tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(run()){eprintln!("期刊开发服务：{e}");std::process::exit(1)}}
#[cfg(not(debug_assertions))]
fn main(){std::process::exit(1)}
#[cfg(debug_assertions)]
async fn run()->Result<(),Box<dyn std::error::Error>>{
    use std::{io::BufRead,net::SocketAddr,sync::Arc};
    use axum::{Router,Json,extract::{State,ConnectInfo,Request,DefaultBodyLimit},middleware::{self,Next},response::Response,routing::post};
    use serde_json::{json,Value};
    use scidesk_lib::journals::{Service,response};
    let mut line=String::new();std::io::stdin().lock().read_line(&mut line)?;
    let options:Value=serde_json::from_str(&line)?;
    let token=options["control_token"].as_str().filter(|s|s.len()==64).ok_or("缺少本地凭证")?.to_owned();
    let app_root=scidesk_lib::project_service::application_root()?;
    let root=if let Some(path)=options["test_root"].as_str(){let p=std::path::PathBuf::from(path);let p=std::fs::canonicalize(p)?;if !p.starts_with(std::fs::canonicalize(app_root.join("development/verification"))?){return Err("测试数据目录必须位于第二版 development/verification 内".into())}p}else{app_root.join("runtime/browser")};
    let service=Service::open(&root.join("journals")).await?;
    service.start_recommendations();
    #[derive(Clone)]struct Dev{service:Arc<Service>,token:String,address:String}
    async fn auth(State(s):State<Dev>,ConnectInfo(peer):ConnectInfo<SocketAddr>,req:Request,next:Next)->Response{
        let headers=req.headers();
        if !peer.ip().is_loopback()||headers.get("authorization").and_then(|x|x.to_str().ok())!=Some(format!("Bearer {}",s.token).as_str())||headers.get("host").and_then(|x|x.to_str().ok())!=Some(s.address.as_str())||headers.contains_key("origin"){
            return Json(response(Err("开发桥接仅供本机应用访问".into()))).into_response()
        }next.run(req).await
    }
    use axum::response::IntoResponse;
    async fn command(State(s):State<Dev>,Json(v):Json<Value>)->Json<Value>{Json(response(s.service.command(v["op"].as_str().unwrap_or(""),v.get("args").cloned().unwrap_or(json!({}))).await))}
    let listener=tokio::net::TcpListener::bind(("127.0.0.1",0)).await?;let address=listener.local_addr()?;
    let state=Dev{service,token,address:address.to_string()};
    let router=Router::new().route("/command",post(command)).layer(DefaultBodyLimit::max(3*1024*1024)).layer(middleware::from_fn_with_state(state.clone(),auth)).with_state(state);
    println!("{}",json!({"port":address.port()}));
    axum::serve(listener,router.into_make_service_with_connect_info::<SocketAddr>()).await?;Ok(())
}
