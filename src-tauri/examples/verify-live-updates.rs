//! Exercise the real updater as the previous release, without installing.
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;
fn main() {
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.package_info_mut().version = "0.3.7".parse().unwrap();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let result = async {
                    let updater = app.updater_builder()
                        .endpoints(vec!["https://github.com/luckySpro/macntfs/releases/latest/download/latest-installer.json".parse().unwrap()])
                        .map_err(|e| e.to_string())?
                        .timeout(Duration::from_secs(120)).build().map_err(|e| e.to_string())?;
                    let update = updater.check().await.map_err(|e| e.to_string())?
                        .ok_or("expected an update for the 0.3.7 client")?;
                    if update.version != env!("CARGO_PKG_VERSION") {
                        return Err(format!("GitHub latest reports {}, expected {}; retry after release propagation", update.version, env!("CARGO_PKG_VERSION")));
                    }
                    println!("PASS: 0.3.7 detects {} at {}", update.version, update.download_url);
                    let data = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
                    let expected = std::fs::read(format!("dist/macntfs-{}-arm64.pkg", update.version)).map_err(|e| e.to_string())?;
                    if !data.starts_with(b"xar!") || data != expected {
                        return Err("downloaded PKG differs from local published bytes".into());
                    }
                    println!("PASS: real Tauri download + version-bound signature + exact released PKG bytes; no installation performed");
                    Ok::<(), String>(())
                }.await;
                if let Err(error) = &result { eprintln!("FAIL: {error}"); }
                app.exit(if result.is_ok() { 0 } else { 1 });
            });
            Ok(())
        })
        .build(context).unwrap().run(|_, _| {});
}
