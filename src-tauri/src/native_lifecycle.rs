//! Cocoa Dock Quit bypasses Tauri ExitRequested; cancel it at the delegate.
use std::{
    ffi::{c_char, c_void},
    sync::OnceLock,
};
use tauri::Manager;
type Ref = *mut c_void;
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Ref;
    fn sel_registerName(name: *const c_char) -> Ref;
    fn object_getClass(object: Ref) -> Ref;
    fn class_addMethod(
        class: Ref,
        selector: Ref,
        implementation: unsafe extern "C" fn(),
        types: *const c_char,
    ) -> bool;
    fn objc_msgSend();
}
// Let system shutdown, restart and logout finish; only ordinary Quit hides.
unsafe fn system_termination() -> bool {
    unsafe {
        let send: unsafe extern "C" fn(Ref, Ref) -> Ref =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let descriptor: unsafe extern "C" fn(Ref, Ref, u32) -> Ref =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let code: unsafe extern "C" fn(Ref, Ref) -> u32 =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let manager = send(
            objc_getClass(c"NSAppleEventManager".as_ptr()),
            sel_registerName(c"sharedAppleEventManager".as_ptr()),
        );
        let event = send(manager, sel_registerName(c"currentAppleEvent".as_ptr()));
        let reason = descriptor(
            event,
            sel_registerName(c"paramDescriptorForKeyword:".as_ptr()),
            u32::from_be_bytes(*b"why?"),
        );
        let reason = code(reason, sel_registerName(c"enumCodeValue".as_ptr()));
        [*b"shut", *b"rest", *b"rlgo", *b"logo", *b"quia"]
            .iter()
            .any(|value| reason == u32::from_be_bytes(*value))
    }
}
extern "C" fn should_terminate(_: Ref, _: Ref, _: Ref) -> usize {
    if unsafe { system_termination() } {
        return 1; // NSTerminateNow
    }
    if let Some(app) = APP.get()
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.hide();
    }
    0 // NSTerminateCancel; explicit tray exit/restart uses Tauri's event loop.
}
pub fn install(app: tauri::AppHandle) -> Result<(), String> {
    APP.set(app).map_err(|_| "窗口生命周期已经初始化")?;
    unsafe {
        let send: unsafe extern "C" fn(Ref, Ref) -> Ref =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let application = send(
            objc_getClass(c"NSApplication".as_ptr()),
            sel_registerName(c"sharedApplication".as_ptr()),
        );
        let delegate = send(application, sel_registerName(c"delegate".as_ptr()));
        if delegate.is_null() {
            return Err("无法配置 Dock 生命周期".into());
        }
        let implementation: unsafe extern "C" fn() =
            std::mem::transmute(should_terminate as extern "C" fn(Ref, Ref, Ref) -> usize);
        if !class_addMethod(
            object_getClass(delegate),
            sel_registerName(c"applicationShouldTerminate:".as_ptr()),
            implementation,
            c"Q@:@".as_ptr(),
        ) {
            return Err("无法注册 Dock 退出处理".into());
        }
    }
    Ok(())
}
