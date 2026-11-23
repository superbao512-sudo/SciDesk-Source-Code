fn main() {
    // Tauri's config watcher does not track icon file changes. Rebuild Windows
    // resources as well as the default window icon whenever either changes.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    println!("cargo:rustc-env=SCIDESK_PROJECT_ROOT={}", root.display());
    tauri_build::build()
}
