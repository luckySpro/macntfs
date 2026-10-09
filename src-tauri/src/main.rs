#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod i18n;
mod native_lifecycle;
mod updates;
use macntfs_core::{
    settings::{Backend, Settings, Theme},
    system,
};
use std::{
    collections::HashSet,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{
    Emitter, Manager,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
#[derive(Clone, Default, serde::Serialize)]
struct Monitor {
    volumes: Vec<system::Volume>,
    last_event: String,
    busy: bool,
    last_error: bool,
}
#[derive(serde::Serialize)]
struct Snapshot {
    volumes: Vec<system::Volume>,
    environment: system::Environment,
    settings: Settings,
    version: &'static str,
    monitor: Monitor,
    required_update: Option<updates::Available>,
}
#[tauri::command]
async fn snapshot(app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(Snapshot {
            required_update: updates::required(&app),
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
        monitor.last_error = error;
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
    updates::guard_operation(&action, updates::required(&app).is_some())?;
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
#[derive(Default)]
struct SettingsState(Mutex<()>);
// Keep Cocoa title text and titlebar background aligned with the saved palette.
fn apply_window_theme(app: &tauri::AppHandle, theme: Theme) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        let (appearance, color) = match theme {
            Theme::Stone => (
                tauri::Theme::Light,
                tauri::window::Color(250, 250, 248, 255),
            ),
            Theme::Office => (
                tauri::Theme::Light,
                tauri::window::Color(248, 249, 250, 255),
            ),
            Theme::Graphite => (tauri::Theme::Dark, tauri::window::Color(32, 36, 38, 255)),
        };
        window.set_theme(Some(appearance))?;
        window.set_background_color(Some(color))?;
    }
    Ok(())
}
#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let state = app.state::<SettingsState>();
    let _guard = state.0.lock().map_err(|e| e.to_string())?;
    if !["auto", "zh-Hans", "zh-Hant", "en", "ja"].contains(&settings.language.as_str()) {
        return Err("Unsupported language".into());
    }
    settings.save()?;
    let handle = app.clone();
    let theme = settings.theme;
    app.run_on_main_thread(move || {
        let _ = apply_window_theme(&handle, theme);
    })
    .map_err(|e| e.to_string())?;
    let _ = app.emit("devices-changed", ());
    Ok(())
}
#[tauri::command]
fn set_auto_mount(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<SettingsState>();
    let _guard = state.0.lock().map_err(|e| e.to_string())?;
    let mut settings = Settings::load();
    settings.auto_mount = enabled;
    settings.save()?;
    let _ = app.emit("devices-changed", ());
    Ok(())
}
fn show_window(app: &tauri::AppHandle) {
    if let Some(panel) = app.get_webview_window("tray-panel") {
        let _ = panel.hide();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
#[derive(Default)]
struct PanelState {
    last_blur: Mutex<Option<Instant>>,
}
#[tauri::command]
fn panel_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("tray-panel") {
        let _ = window.hide();
    }
    match action.as_str() {
        "hide" => {}
        "show" => show_window(&app),
        "settings" => {
            show_window(&app);
            let _ = app.emit_to("main", "navigate-settings", ());
        }
        "quit" => app.exit(0),
        _ => return Err("无效窗口操作".into()),
    }
    Ok(())
}
fn toggle_panel(app: &tauri::AppHandle, position: tauri::PhysicalPosition<f64>, rect: tauri::Rect) {
    let Some(window) = app.get_webview_window("tray-panel") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if app
        .state::<PanelState>()
        .last_blur
        .lock()
        .ok()
        .and_then(|v| *v)
        .is_some_and(|t| t.elapsed() < Duration::from_millis(180))
    {
        return;
    }
    let monitors = window.available_monitors().unwrap_or_default();
    if let Some(monitor) = monitors.into_iter().find(|m| {
        let p = m.position();
        let size = m.size();
        position.x >= p.x as f64
            && position.x < p.x as f64 + size.width as f64
            && position.y >= p.y as f64
            && position.y < p.y as f64 + size.height as f64
    }) {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let width = 420.0_f64.min(area.size.width as f64 / scale - 16.0);
        let height = 540.0_f64.min(area.size.height as f64 / scale - 16.0);
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
        let origin = rect.position.to_physical::<f64>(scale);
        let size = rect.size.to_physical::<f64>(scale);
        let x = (origin.x + size.width / 2.0 - width * scale / 2.0).clamp(
            area.position.x as f64 + 8.0,
            area.position.x as f64 + area.size.width as f64 - width * scale - 8.0,
        );
        let y = (origin.y + size.height + 6.0).clamp(
            area.position.y as f64 + 4.0,
            area.position.y as f64 + area.size.height as f64 - height * scale - 4.0,
        );
        let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    }
    let _ = app.emit_to("tray-panel", "devices-changed", ());
    let _ = window.show();
    let _ = window.set_focus();
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
        i18n::text("打开 macntfs"),
        true,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "auto",
        i18n::text("插入 NTFS 磁盘后自动开启读写"),
        true,
        Settings::load().auto_mount,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "service-status",
        if ready {
            i18n::text("后台助手已就绪 · 挂载无需密码")
        } else {
            i18n::text("请安装组件并从应用程序启动")
        },
        false,
        None::<&str>,
    )?)?;
    if let Some(update) = updates::required(app) {
        menu.append(&MenuItem::with_id(
            app,
            "show-update",
            format!("{} · v{}", i18n::text("需要更新"), update.version),
            true,
            None::<&str>,
        )?)?;
    }
    for volume in &monitor.volumes {
        let state = if volume.mount.is_empty() {
            i18n::text("未挂载")
        } else if volume.writable {
            i18n::text("可读写")
        } else {
            i18n::text("只读")
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
            i18n::text("在 Finder 中打开"),
            !monitor.busy && !volume.mount.is_empty(),
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("mount:{}", volume.uuid),
            i18n::text("开启读写"),
            ready
                && updates::required(app).is_none()
                && !monitor.busy
                && (!volume.writable || volume.mount.is_empty()),
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            format!("eject:{}", volume.uuid),
            i18n::text("安全推出整块磁盘"),
            !monitor.busy,
            None::<&str>,
        )?)?;
    }
    if monitor.volumes.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "empty",
            i18n::text("等待连接 NTFS 磁盘"),
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
        i18n::text("退出 macntfs"),
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}
fn menu_action(app: &tauri::AppHandle, id: &str) {
    match id {
        "show" | "show-update" => show_window(app),
        "quit" => app.exit(0),
        "auto" => {
            let state = app.state::<SettingsState>();
            let result = state
                .0
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|_guard| {
                    let mut settings = Settings::load();
                    settings.auto_mount = !settings.auto_mount;
                    settings.save().map(|_| {
                        if settings.auto_mount {
                            "已开启插入自动读写"
                        } else {
                            "已关闭自动读写"
                        }
                        .into()
                    })
                });
            let (message, error) = match result {
                Ok(message) => (message, false),
                Err(message) => (message, true),
            };
            if let Ok(mut monitor) = app.state::<Mutex<Monitor>>().lock() {
                monitor.last_event = message.clone();
                monitor.last_error = error;
            }
            let _ = app.emit(
                "operation-result",
                serde_json::json!({"message":message,"error":error}),
            );
            let _ = app.emit("devices-changed", ());
        }
        _ => {
            if let Some((action, uuid)) = id.split_once(':') {
                if updates::guard_operation(action, updates::required(app).is_some()).is_err() {
                    show_window(app);
                    return;
                }
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
                if settings.auto_mount && ready && updates::required(&app).is_none() {
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
                let key = serde_json::to_string(&(
                    &monitor,
                    ready,
                    settings.auto_mount,
                    &settings.language,
                    updates::required(&app),
                ))
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
        .manage(PanelState::default())
        .manage(SettingsState::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle();
            native_lifecycle::install(handle.clone())?;
            apply_window_theme(handle, Settings::load().theme)?;
            updates::restore(handle);
            let updater_app = handle.clone();
            std::thread::spawn(move || {
                loop {
                    let _ = tauri::async_runtime::block_on(updates::check_updates(
                        updater_app.clone(),
                        updater_app.state::<updates::Pending>(),
                    ));
                    // Recheck periodically even while the main window stays hidden.
                    std::thread::sleep(Duration::from_secs(3600));
                }
            });
            let tray = TrayIconBuilder::with_id("macntfs")
                .icon(tray_image())
                .icon_as_template(true)
                .tooltip("macntfs · NTFS 自动读写")
                .menu(&tray_menu(handle, &Monitor::default(), false)?)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        position,
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_panel(tray.app_handle(), position, rect);
                    }
                })
                .on_menu_event(|app, event| menu_action(app, event.id.as_ref()))
                .build(app)?;
            #[cfg(debug_assertions)]
            if std::env::var_os("MACNTFS_PREVIEW_PANEL").is_some() {
                if let Some(main) = handle.get_webview_window("main") {
                    let _ = main.hide();
                }
                if let Some(rect) = tray.rect()? {
                    let scale = handle
                        .primary_monitor()?
                        .map(|m| m.scale_factor())
                        .unwrap_or(1.0);
                    let point = rect.position.to_physical::<f64>(scale);
                    toggle_panel(handle, point, rect);
                }
            }
            #[cfg(not(debug_assertions))]
            let _ = tray;
            start_monitor(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "tray-panel"
                && matches!(event, tauri::WindowEvent::Focused(false))
                && window.is_visible().unwrap_or(false)
            {
                let _ = window.hide();
                if let Ok(mut last) = window.app_handle().state::<PanelState>().last_blur.lock() {
                    *last = Some(Instant::now());
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            operate,
            save_settings,
            panel_action,
            set_auto_mount,
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
                for label in ["main", "tray-panel"] {
                    if let Some(window) = app.get_webview_window(label) {
                        let _ = window.hide();
                    }
                }
            }
            _ => {}
        });
}
