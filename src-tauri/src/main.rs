#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use ntfs_desktop::{
    settings::{Backend, Settings},
    system,
};
#[derive(serde::Serialize)]
struct Snapshot {
    volumes: Vec<system::Volume>,
    environment: system::Environment,
    settings: Settings,
    version: &'static str,
}
#[tauri::command]
async fn snapshot() -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok(Snapshot {
            volumes: system::scan()?,
            environment: system::environment(),
            settings: Settings::load(),
            version: env!("CARGO_PKG_VERSION"),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn operate(
    action: String,
    volume: Option<system::Volume>,
    backend: Backend,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "mount" => system::mount(&volume.ok_or("请选择磁盘")?, backend),
        "eject" => system::eject(&volume.ok_or("请选择磁盘")?),
        "open" => system::open_volume(&volume.ok_or("请选择磁盘")?),
        "install" => system::install(),
        "settings" => system::open_settings(),
        _ => Err("无效操作".into()),
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings.save()
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![snapshot, operate, save_settings])
        .run(tauri::generate_context!())
        .expect("启动 NTFS Desktop 失败");
}
