use super::*;
#[test]
fn png_and_windows_dib_preserve_pixel_dimensions_channels_and_orientation(){
 let pixels=vec![255,0,0,255,0,255,0,255,0,0,255,255,25,125,225,255];
 let png=encode_png(2,2,&pixels).unwrap();assert_eq!(decode_png(&png).unwrap(),(2,2,pixels.clone()));
 let dib=dibv5(2,2,&pixels).unwrap();assert_eq!(&dib[124..128],&[0,0,255,255]);
 assert_eq!(decode_png(&decode_dib(&dib).unwrap()).unwrap(),(2,2,pixels));
 assert!(decode_dib(&dib[..130]).is_err());assert!(decode_png(b"not a PNG").is_err());
 assert!(checked_pixels(0,1).is_err());assert!(checked_pixels(100000,100000).is_err());
}
#[test]
fn negative_monitor_origins_and_partial_window_intersections_are_physical_pixels(){
 let desktop=Rect{x:-1920,y:-300,width:4480,height:1740};let window=Rect{x:-2000,y:100,width:800,height:600};
 assert_eq!(window.intersection(desktop),Some(Rect{x:-1920,y:100,width:720,height:600}));
 assert_eq!(Rect{x:9000,y:0,width:100,height:100}.intersection(desktop),None);
}
#[test]
fn shortcut_validation_rejects_ambiguous_and_reserved_combinations(){
 assert_eq!(hotkey::parse("Ctrl+Shift+F6").unwrap(),(6,117));assert_eq!(hotkey::parse("Alt+Ctrl+A").unwrap(),(3,65));
 for input in ["S","Shift+A","Win+Shift+S","Alt+F4","Ctrl+W","Ctrl+Ctrl+A","Ctrl+A+B","Ctrl+F13","Ctrl+eval()"]{assert!(hotkey::parse(input).is_err(),"{input}");}
 assert!(hotkey::validate(&Settings{delay:2,..Default::default()}).is_err());
}
#[test]
fn explicit_png_save_keeps_references_read_only_and_supports_chinese_paths(){
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(format!("development/verification/screenshot-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
 let file=root.join("像素检查.png");let png=encode_png(1,1,&[1,2,3,255]).unwrap();write_saved_png(&file,&png).unwrap();assert_eq!(std::fs::read(&file).unwrap(),png);
 assert!(write_saved_png(&root.join("invalid.jpg"),&png).is_err());
 assert!(write_saved_png(Path::new("E:/SciDesk/should-never-be-created.png"),&png).is_err());
 std::fs::remove_file(file).unwrap();std::fs::remove_dir(root).unwrap();
}
#[cfg(windows)]
#[test]
fn windows_capture_reads_actual_virtual_desktop_without_writing_images(){
 let frame=win::capture().expect("actual Windows desktop capture");
 let (width,height,pixels)=decode_png(&frame.png).unwrap();assert_eq!((width,height),(frame.rect.width,frame.rect.height));assert!(!frame.monitors.is_empty());
 assert!(pixels.chunks_exact(4).any(|p|p[..3]!=pixels[..3]),"desktop capture unexpectedly contains one flat color");
 eprintln!("Windows capture: {}x{} physical pixels, origin ({}, {}), {} monitors",width,height,frame.rect.x,frame.rect.y,frame.monitors.len());
}
