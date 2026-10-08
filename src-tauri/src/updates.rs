//! Signed full-installer updates keep the GUI and privileged runtime together.
use std::{io::Write, sync::Mutex, time::Duration};
use tauri::Emitter;
use tauri_plugin_updater::{Update, UpdaterExt};
#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>);
#[derive(serde::Serialize)]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
}
#[tauri::command]
pub async fn check_updates(
    app: tauri::AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<Option<Available>, String> {
    let updater = app
        .updater_builder()
        .endpoints(vec![
            "https://github.com/luckySpro/macntfs/releases/latest/download/latest-installer.json"
                .parse()
                .map_err(|_| "更新地址无效")?,
        ])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    if let Some(update) = &update {
        let expected = format!(
            "https://github.com/luckySpro/macntfs/releases/download/v{0}/macntfs-{0}-arm64.pkg",
            update.version
        );
        if update.download_url.as_str() != expected {
            return Err("完整更新包地址不匹配".into());
        }
    }
    let available = update.as_ref().map(|u| Available {
        version: u.version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().map_err(|_| "更新状态不可用")? = update;
    Ok(available)
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .map_err(|_| "更新状态不可用")?
        .take()
        .ok_or("请先检查更新")?;
    let mut received = 0u64;
    let bytes = update
        .download(
            |size, total| {
                received += size as u64;
                let _ = app.emit(
                    "update-progress",
                    serde_json::json!({"received":received,"total":total}),
                );
            },
            || {},
        )
        .await
        .map_err(|e| format!("更新下载或签名验证失败：{e}"))?;
    // Unique private staging path, only written after version-bound signature verification.
    let mut file = tempfile::Builder::new()
        .prefix("macntfs-update-")
        .suffix(".pkg")
        .tempfile()
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    let (_, path) = file.keep().map_err(|e| e.to_string())?;
    let result = std::process::Command::new("/usr/bin/open")
        .args(["-a", "Installer"])
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    if !result.success() {
        return Err("无法打开系统安装器，请重试".into());
    }
    // Do not keep an old GUI alive while Installer replaces its app bundle.
    app.exit(0);
    Ok(())
}
