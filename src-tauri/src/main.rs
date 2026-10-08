#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod native_lifecycle;
mod updates;
use macntfs_core::{
    settings::{Backend, Settings},
    system,
};
use std::{collections::HashSet, sync::Mutex, time::Duration};
use tauri::{
    Emitter, Manager,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
#[derive(Clone, Default, serde::Serialize)]
struct Monitor {
    volumes: Vec<system::Volume>,
    last_event: String,
    busy: bool,
}
#[derive(serde::Serialize)]
struct Snapshot {
    volumes: Vec<system::Volume>,
    environment: system::Environment,
    settings: Settings,
    version: &'static str,
    monitor: Monitor,
}
#[tauri::command]
async fn snapshot(app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(Snapshot {
            volumes: system::scan()?,
            environment: system::environment(),
            settings: Settings::load(),
            version: env!("CARGO_PKG_VERSION"),
            monitor: app
                .state::<Mutex<Monitor>>()
                .lock()
                .map_err(|e| e.to_string())?
                .clone(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
fn operation(
    action: &str,
    volume: Option<system::Volume>,
    backend: Backend,
) -> Result<String, String> {
    match action {
        "mount" => system::mount(&volume.ok_or("请选择磁盘")?, backend),
        "eject" => system::eject(&volume.ok_or("请选择磁盘")?),
        "open" => system::open_volume(&volume.ok_or("请选择磁盘")?),
        "install" => system::install(),
        "release" => system::open_release(),
        "guide-macfuse" => system::open_guide(false),
        "guide-apple" => system::open_guide(true),
        "settings" => system::open_settings(),
        "permissions" => system::open_disk_permissions(),
        "permission-helper" => system::reveal_permission_helper(),
        _ => Err("无效操作".into()),
    }
}
fn report(app: &tauri::AppHandle, result: Result<String, String>) {
    let (message, error) = match result {
        Ok(s) => (s, false),
        Err(s) => (s, true),
    };
    if let Ok(mut monitor) = app.state::<Mutex<Monitor>>().lock() {
        monitor.last_event = message.clone();
        monitor.busy = false;
    }
    let _ = app.emit(
        "operation-result",
        serde_json::json!({"message":message,"error":error}),
    );
}
#[tauri::command]
async fn operate(
    app: tauri::AppHandle,
    action: String,
    volume: Option<system::Volume>,
    backend: Backend,
) -> Result<String, String> {
    {
        let state = app.state::<Mutex<Monitor>>();
        let mut monitor = state.lock().map_err(|e| e.to_string())?;
        if monitor.busy {
            return Err("已有磁盘操作正在执行，请稍后重试".into());
        }
        monitor.busy = true;
    }
    let _ = app.emit("devices-changed", ());
    tauri::async_runtime::spawn_blocking(move || {
        let result = operation(&action, volume, backend);
        report(&app, result.clone());
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings.save()
}
fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn tray_menu(
    app: &tauri::AppHandle,
    monitor: &Monitor,
    ready: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        "打开 macntfs",
        true,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "auto",
        "插入 NTFS 磁盘后自动开启读写",
        true,
        Settings::load().auto_mount,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "service-status",
        if ready {
            "后台助手已就绪 · 挂载无需密码"
        } else {
            "请安装组件并从应用程序启动"
        },
        false,
        None::<&str>,
    )?)?;
    for volume in &monitor.volumes {
        let state = if volume.mount.is_empty() {
            "未挂载"
        } else if volume.writable {
            "可读写"
        } else {
            "只读"
        };
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("volume:{}", volume.uuid),
            format!("{} · {}", volume.name, state),
            false,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("open:{}", volume.uuid),
            "在 Finder 中打开",
            !monitor.busy && !volume.mount.is_empty(),
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("mount:{}", volume.uuid),
            "开启读写",
            ready && !monitor.busy && (!volume.writable || volume.mount.is_empty()),
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("eject:{}", volume.uuid),
            "安全推出整块磁盘",
            !monitor.busy,
            None::<&str>,
        )?)?;
    }
    if monitor.volumes.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "empty",
            "等待连接 NTFS 磁盘",
            false,
            None::<&str>,
        )?)?;
    }
    if !monitor.last_event.is_empty() {
        let title: String = monitor
            .last_event
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(55)
            .collect();
        menu.append(&MenuItem::with_id(app, "last", title, false, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "退出 macntfs",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}
fn menu_action(app: &tauri::AppHandle, id: &str) {
    match id {
        "show" => show_window(app),
        "quit" => app.exit(0),
        "auto" => {
            let mut settings = Settings::load();
            settings.auto_mount = !settings.auto_mount;
            let result = settings.save().map(|_| {
                if settings.auto_mount {
                    "已开启插入自动读写"
                } else {
                    "已关闭自动读写"
                }
                .into()
            });
            let (message, error) = match result {
                Ok(message) => (message, false),
                Err(message) => (message, true),
            };
            if let Ok(mut monitor) = app.state::<Mutex<Monitor>>().lock() {
                monitor.last_event = message.clone();
            }
            let _ = app.emit(
                "operation-result",
                serde_json::json!({"message":message,"error":error}),
            );
            let _ = app.emit("devices-changed", ());
        }
        _ => {
            if let Some((action, uuid)) = id.split_once(':') {
                if !matches!(action, "open" | "mount" | "eject") {
                    return;
                }
                let volume = {
                    let state = app.state::<Mutex<Monitor>>();
                    let Ok(mut monitor) = state.lock() else {
                        return;
                    };
                    if monitor.busy {
                        return;
                    }
                    let Some(volume) = monitor.volumes.iter().find(|v| v.uuid == uuid).cloned()
                    else {
                        return;
                    };
                    monitor.busy = true;
                    Some(volume)
                };
                let _ = app.emit("devices-changed", ());
                let app = app.clone();
                let action = action.to_owned();
                tauri::async_runtime::spawn_blocking(move || {
                    let result = operation(&action, volume, Settings::load().backend);
                    report(&app, result);
                });
            }
        }
    }
}
fn start_monitor(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let events = macntfs_core::devices::notifications();
        let mut attempted = HashSet::new();
        let mut was_enabled = Settings::load().auto_mount;
        let mut last_menu = String::new();
        loop {
            let settings = Settings::load();
            if settings.auto_mount && !was_enabled {
                attempted.clear();
            }
            was_enabled = settings.auto_mount;
            let ready = macntfs_core::daemon::ready();
            if let Ok(volumes) = system::scan() {
                let present: HashSet<String> = volumes.iter().map(|v| v.uuid.clone()).collect();
                attempted.retain(|id| present.contains(id));
                if let Ok(mut monitor) = app.state::<Mutex<Monitor>>().lock() {
                    monitor.volumes = volumes.clone();
                }
                if settings.auto_mount && ready {
                    for volume in &volumes {
                        if (!volume.writable || volume.mount.is_empty())
                            && !attempted.contains(&volume.uuid)
                        {
                            let claimed = app
                                .state::<Mutex<Monitor>>()
                                .lock()
                                .map(|mut monitor| {
                                    if monitor.busy {
                                        false
                                    } else {
                                        monitor.busy = true;
                                        true
                                    }
                                })
                                .unwrap_or(false);
                            if !claimed {
                                break;
                            }
                            attempted.insert(volume.uuid.clone());
                            let _ = app.emit("devices-changed", ());
                            report(&app, system::mount(volume, settings.backend));
                        }
                    }
                }
                let monitor = app
                    .state::<Mutex<Monitor>>()
                    .lock()
                    .map(|m| m.clone())
                    .unwrap_or_default();
                let key = serde_json::to_string(&(&monitor, ready, settings.auto_mount))
                    .unwrap_or_default();
                if key != last_menu {
                    last_menu = key;
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let (Some(tray), Ok(menu)) = (
                            handle.tray_by_id("macntfs"),
                            tray_menu(&handle, &monitor, ready),
                        ) {
                            let _ = tray.set_menu(Some(menu));
                        }
                    });
                    let _ = app.emit("devices-changed", ());
                }
            }
            let _ = events.recv_timeout(Duration::from_secs(2));
        }
    });
}
fn tray_image() -> tauri::image::Image<'static> {
    let mut rgba = vec![0; 18 * 18 * 4];
    for y in 2..16 {
        for x in 2..16 {
            if x == 2 || x == 15 || y == 2 || y == 15 || y == 11 || (y == 13 && x >= 12) {
                rgba[(y * 18 + x) * 4 + 3] = 255;
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 18, 18)
}
fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(Monitor::default()))
        .manage(updates::Pending::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle();
            native_lifecycle::install(handle.clone())?;
            TrayIconBuilder::with_id("macntfs")
                .icon(tray_image())
                .icon_as_template(true)
                .tooltip("macntfs · NTFS 自动读写")
                .menu(&tray_menu(handle, &Monitor::default(), false)?)
                .on_menu_event(|app, event| menu_action(app, event.id.as_ref()))
                .build(app)?;
            start_monitor(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            operate,
            save_settings,
            updates::check_updates,
            updates::install_update
        ])
        .build(tauri::generate_context!())
        .expect("启动 macntfs 失败")
        .run(|app, event| match event {
            // Dock click / Dock Open must restore the hidden main window.
            tauri::RunEvent::Reopen { .. } => show_window(app),
            // Native Dock Quit and Cmd-Q hide the UI but preserve monitoring.
            // Explicit tray Quit and updater restart use Some(code) and exit.
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } => {
                api.prevent_exit();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        });
}
