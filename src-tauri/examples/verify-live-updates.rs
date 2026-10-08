//! Exercise the real updater against GitHub as a 0.3.6 client, without installing.
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;
fn main() {
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.package_info_mut().version = "0.3.6".parse().unwrap();
    tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app=app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let result=async {
                    let updater=app.updater_builder().endpoints(vec!["https://github.com/luckySpro/macntfs/releases/latest/download/latest-installer.json".parse().unwrap()])?.timeout(Duration::from_secs(120)).build()?;
                    let update=updater.check().await?.expect("expected newer installer than 0.3.6");
                    println!("PASS: 0.3.6 detects {} at {}",update.version,update.download_url);
                    let data=update.download(|_,_|{},||{}).await?;
                    assert!(data.starts_with(b"xar!"));
                    let expected=std::fs::read(format!("dist/macntfs-{}-arm64.pkg",update.version)).unwrap();
                    assert_eq!(data,expected);
                    println!("PASS: real Tauri download + version-bound signature + exact released PKG bytes; no installation performed");
                    Ok::<(),tauri_plugin_updater::Error>(())
                }.await;
                if let Err(error)=&result { eprintln!("FAIL: {error}"); }
                app.exit(if result.is_ok(){0}else{1});
            });
            Ok(())
        }).build(context).unwrap().run(|_,_|{});
}
