use serde::{Deserialize,Serialize};
use serde_json::Value;
pub fn defaults()->Value{serde_json::from_str(include_str!("../../src/features/settings/defaults.json")).expect("bundled settings defaults")}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct WindowState{pub width:f64,pub height:f64,pub x:i32,pub y:i32,pub maximized:bool}
#[derive(Clone,Debug)]
pub struct Screen{pub x:i32,pub y:i32,pub width:u32,pub height:u32,pub scale:f64}
pub fn parse_preferences(rows:Vec<(String,String)>)->Value{
 let mut config=defaults()["preferences"].clone();
 for (key,text) in rows{if !key.starts_with("startup.")||config.get(&key).is_none(){continue}let parsed=serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));let good=match key.as_str(){"startup.splash_mode"=>parsed.as_str().is_some_and(|x|["follow_system","always_video","minimal_logo","disabled"].contains(&x)),"startup.default_width"=>parsed.as_f64().is_some_and(|x|(1050.0..=3840.0).contains(&x)),"startup.default_height"=>parsed.as_f64().is_some_and(|x|(680.0..=2160.0).contains(&x)),_=>parsed.is_boolean()};if good{config[&key]=parsed}}
 config
}
pub fn visible_state(state:&WindowState,screens:&[Screen])->bool{
 if !state.width.is_finite()||!state.height.is_finite()||state.width<=0.0||state.height<=0.0||state.width>3840.0||state.height>2160.0{return false}
 screens.iter().any(|s|{let width=state.width*s.scale;let height=(state.height+40.0)*s.scale;state.x>=s.x&&state.y>=s.y&&state.x as f64+width<=s.x as f64+s.width as f64+16.0&&state.y as f64+height<=s.y as f64+s.height as f64+16.0})
}
