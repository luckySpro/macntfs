//! Signed full-installer updates keep the GUI and privileged runtime together.
use std::{io::Write, sync::Mutex, time::Duration};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>, pub Mutex<Option<Available>>);
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
}
fn newer_than(version: &str, baseline: &str) -> bool {
    let parse = |v: &str| {
        v.split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
    };
    match (parse(version), parse(baseline)) {
        (Ok(a), Ok(b)) if a.len() == 3 && b.len() == 3 => a > b,
        _ => false,
    }
}
fn newer(version: &str) -> bool {
    newer_than(version, env!("CARGO_PKG_VERSION"))
}
pub fn guard_operation(action: &str, update_required: bool) -> Result<(), String> {
    if action == "mount" && update_required {
        Err("需要更新 macntfs 后才能开启读写".into())
    } else {
        Ok(())
    }
}
pub fn required(app: &tauri::AppHandle) -> Option<Available> {
    app.state::<Pending>().1.lock().ok().and_then(|v| v.clone())
}
pub fn restore(app: &tauri::AppHandle) {
    let path = macntfs_core::settings::data_dir().join("required-update.json");
    if let Ok(bytes) = std::fs::read(&path)
        && let Ok(update) = serde_json::from_slice::<Available>(&bytes)
    {
        if newer(&update.version) {
            *app.state::<Pending>().1.lock().unwrap() = Some(update);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
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
    let mut update = updater.check().await.map_err(|e| e.to_string())?;
    // A latest-release rollback must not strand a previously recorded requirement.
    if let Some(known) = required(&app)
        && update
            .as_ref()
            .is_none_or(|u| newer_than(&known.version, &u.version))
    {
        let endpoint = format!(
            "https://github.com/luckySpro/macntfs/releases/download/v{}/latest-installer.json",
            known.version
        );
        update = app
            .updater_builder()
            .endpoints(vec![endpoint.parse().map_err(|_| "Invalid update URL")?])
            .map_err(|e| e.to_string())?
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())?;
    }
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
    if let Some(available) = &available {
        *pending.1.lock().map_err(|_| "更新状态不可用")? = Some(available.clone());
        let dir = macntfs_core::settings::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut cache = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
        cache
            .write_all(&serde_json::to_vec(available).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        cache.as_file().sync_all().map_err(|e| e.to_string())?;
        cache
            .persist(dir.join("required-update.json"))
            .map_err(|e| e.to_string())?;
        let _ = app.emit("devices-changed", ());
    }
    *pending.0.lock().map_err(|_| "更新状态不可用")? = update;
    Ok(available)
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<(), String> {
    if app
        .state::<Mutex<super::Monitor>>()
        .lock()
        .map_err(|e| e.to_string())?
        .busy
    {
        return Err("Wait for the current disk operation before updating".into());
    }
    if pending
        .0
        .lock()
        .map_err(|_| "Update state unavailable")?
        .is_none()
    {
        check_updates(app.clone(), app.state::<Pending>()).await?;
    }
    let update = pending
        .0
        .lock()
        .map_err(|_| "更新状态不可用")?
        .clone()
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_update_blocks_mount_but_preserves_safe_eject_and_open() {
        assert!(guard_operation("mount", true).is_err());
        assert!(guard_operation("mount", false).is_ok());
        assert!(guard_operation("eject", true).is_ok());
        assert!(guard_operation("open", true).is_ok());
    }
    #[test]
    fn mandatory_requirement_clears_only_after_installing_newer_release() {
        assert!(!newer(env!("CARGO_PKG_VERSION")));
        assert!(!newer("0.0.1"));
        assert!(newer("99.0.0"));
        assert!(!newer("not-a-version"));
        assert!(!newer("999"));
        assert!(newer_than("0.3.10", "0.3.9"));
        assert!(!newer_than("0.3.9", "0.3.10"));
        let cached = Available {
            version: "99.0.0".into(),
            notes: None,
        };
        let restored: Available =
            serde_json::from_slice(&serde_json::to_vec(&cached).unwrap()).unwrap();
        assert!(newer(&restored.version));
    }
}
