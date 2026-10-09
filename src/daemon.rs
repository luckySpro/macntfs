//! Root service: authenticated local GUI requests, never shells or arbitrary paths.
use crate::{
    privileged,
    system::{RUNTIME, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, c_void},
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
pub const SOCKET: &str = "/var/run/com.macntfs.helper.sock";
const CLIENT: &str = "/Applications/macntfs.app/Contents/MacOS/macntfs";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: String,
    id: String,
    uuid: String,
    backend: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    ok: bool,
    message: String,
    version: String,
}
fn response(result: Result<String>) -> Response {
    let result = result.map(bounded_message).map_err(bounded_message);
    match result {
        Ok(message) => Response {
            ok: true,
            message,
            version: env!("CARGO_PKG_VERSION").into(),
        },
        Err(message) => Response {
            ok: false,
            message,
            version: env!("CARGO_PKG_VERSION").into(),
        },
    }
}
fn bounded_message(message: String) -> String {
    let mut bounded = String::new();
    for ch in message.chars() {
        let ch = if ch.is_control() && ch != '\n' && ch != '\t' {
            ' '
        } else {
            ch
        };
        if bounded.len() + ch.len_utf8() > 1280 {
            bounded.push('…');
            break;
        }
        bounded.push(ch);
    }
    bounded
}
fn read_message(stream: &mut UnixStream) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for _ in 0..4096 {
        let mut byte = [0];
        if stream.read(&mut byte).map_err(|e| e.to_string())? == 0 {
            return Err("服务消息不完整".into());
        }
        if byte[0] == b'\n' {
            return Ok(bytes);
        }
        bytes.push(byte[0]);
    }
    Err("服务消息超出限制".into())
}
fn peer(stream: &UnixStream) -> Result<(u32, u32)> {
    let (mut uid, mut gid) = (0, 0);
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err("无法验证服务连接身份".into());
    }
    Ok((uid, gid))
}
pub fn request(action: &str, id: &str, uuid: &str, backend: &str) -> Result<String> {
    let meta = fs::symlink_metadata(SOCKET)
        .map_err(|_| "后台助手未运行，请安装新版组件并从「应用程序」启动 macntfs")?;
    if meta.uid() != 0 || !meta.file_type().is_socket() {
        return Err("后台助手连接不可信".into());
    }
    let mut stream = UnixStream::connect(SOCKET).map_err(|e| e.to_string())?;
    if peer(&stream)?.0 != 0 {
        return Err("后台助手身份不可信".into());
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(if action == "status" {
            5
        } else if backend == "microvm" {
            120
        } else {
            90
        })))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut bytes = serde_json::to_vec(&Request {
        action: action.into(),
        id: id.into(),
        uuid: uuid.into(),
        backend: backend.into(),
    })
    .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(|e| e.to_string())?;
    let result: Response =
        serde_json::from_slice(&read_message(&mut stream)?).map_err(|e| e.to_string())?;
    if result.version != env!("CARGO_PKG_VERSION") {
        return Err(format!(
            "应用版本 {} 与后台助手版本 {} 不一致，请安装对应版本并重新启动应用",
            env!("CARGO_PKG_VERSION"),
            result.version
        ));
    }
    if result.ok {
        Ok(result.message)
    } else {
        Err(result.message)
    }
}
pub fn ready() -> bool {
    request("status", "", "", "").is_ok()
}
// Apple audit tokens bind the connection to the exact process instance (PID reuse safe).
type Ref = *const c_void;
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecGuestAttributeAudit: Ref;
    static kSecCodeInfoFlags: Ref;
    fn SecCodeCopyGuestWithAttributes(host: Ref, attrs: Ref, flags: u32, code: *mut Ref) -> i32;
    fn SecCodeCheckValidity(code: Ref, flags: u32, requirement: Ref) -> i32;
    fn SecCodeCopySigningInformation(code: Ref, flags: u32, information: *mut Ref) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(allocator: Ref, bytes: *const u8, length: isize) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: Ref,
        value_callbacks: Ref,
    ) -> Ref;
    fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: i32, value: *mut c_void) -> bool;
    fn CFRelease(value: Ref);
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}
fn authorize(stream: &UnixStream) -> Result<()> {
    let uid = peer(stream)?.0;
    let console = fs::metadata("/dev/console")
        .map_err(|e| e.to_string())?
        .uid();
    if uid == 0 || uid != console {
        return Err("仅允许当前桌面登录用户使用后台助手".into());
    }
    let mut token = [0u32; 8];
    let mut length = std::mem::size_of_val(&token) as libc::socklen_t;
    // LOCAL_PEERTOKEN = 6 in Darwin sys/un.h.
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            6,
            token.as_mut_ptr().cast(),
            &mut length,
        )
    } != 0
        || length as usize != std::mem::size_of_val(&token)
    {
        return Err("无法验证应用审计身份".into());
    }
    let mut path = [0i8; 4096];
    if unsafe { libc::proc_pidpath(token[5] as i32, path.as_mut_ptr().cast(), path.len() as u32) }
        <= 0
    {
        return Err("客户端已退出".into());
    }
    if unsafe { CStr::from_ptr(path.as_ptr()) }.to_bytes() != CLIENT.as_bytes() {
        return Err("请从「应用程序」启动已安装的 macntfs，后台助手不接受其他程序".into());
    }
    for p in Path::new(CLIENT)
        .ancestors()
        .take_while(|p| *p != Path::new("/Applications"))
    {
        privileged::check_root_path_until_app(p)?;
    }
    let version = plist::Value::from_file("/Applications/macntfs.app/Contents/Info.plist")
        .map_err(|e| e.to_string())?;
    if version
        .as_dictionary()
        .and_then(|d| d.get("CFBundleShortVersionString"))
        .and_then(plist::Value::as_string)
        != Some(env!("CARGO_PKG_VERSION"))
    {
        return Err("应用和后台助手版本不匹配，请更新组件".into());
    }
    unsafe {
        let data = Owned(CFDataCreate(
            std::ptr::null(),
            token.as_ptr().cast(),
            std::mem::size_of_val(&token) as isize,
        ));
        if data.0.is_null() {
            return Err("无法读取应用审计身份".into());
        }
        let attrs = Owned(CFDictionaryCreate(
            std::ptr::null(),
            &kSecGuestAttributeAudit,
            &data.0,
            1,
            std::ptr::null(),
            std::ptr::null(),
        ));
        if attrs.0.is_null() {
            return Err("无法验证应用签名".into());
        }
        let mut code = std::ptr::null();
        if SecCodeCopyGuestWithAttributes(std::ptr::null(), attrs.0, 0, &mut code) != 0 {
            return Err("应用审计签名验证失败".into());
        }
        let code = Owned(code);
        if SecCodeCheckValidity(code.0, 0, std::ptr::null()) != 0 {
            return Err("运行中的应用签名无效".into());
        }
        let mut information = std::ptr::null();
        if SecCodeCopySigningInformation(code.0, 2, &mut information) != 0 {
            return Err("无法验证应用运行时保护".into());
        }
        let information = Owned(information);
        let number = CFDictionaryGetValue(information.0, kSecCodeInfoFlags);
        let mut flags = 0i32;
        if number.is_null()
            || !CFNumberGetValue(number, 3, (&mut flags as *mut i32).cast())
            || flags & 0x10000 == 0
        {
            return Err("应用缺少运行时保护，请使用完整安装包".into());
        }
    }
    Ok(())
}
fn dispatch(req: Request, lock: &Mutex<()>) -> Result<String> {
    match req.action.as_str() {
        "status" if req.id.is_empty() && req.uuid.is_empty() && req.backend.is_empty() => {
            Ok("后台助手已就绪".into())
        }
        "update-check" | "prepare-update" | "cancel-update"
            if req.id.is_empty() && req.uuid.is_empty() && req.backend.is_empty() =>
        {
            let _guard = lock.lock().map_err(|_| "后台挂载锁不可用")?;
            if req.action == "cancel-update" {
                let _ = fs::remove_file("/var/run/com.macntfs.updating");
                return Ok("已恢复设备检测".into());
            }
            privileged::ensure_update_safe()?;
            if req.action == "prepare-update" {
                use std::os::unix::fs::OpenOptionsExt;
                fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open("/var/run/com.macntfs.updating")
                    .map_err(|e| e.to_string())?;
            }
            Ok("磁盘已安全卸载，可以更新".into())
        }
        "eject" if req.backend.is_empty() => {
            let _guard = lock.lock().map_err(|_| "后台挂载锁不可用")?;
            let _active = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open("/var/run/com.macntfs.mount-active")
                .map_err(|e| e.to_string())?;
            privileged::eject(&req.id, &req.uuid)
        }
        "mount" => {
            let _guard = lock.lock().map_err(|_| "后台挂载锁不可用")?;
            let active = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open("/var/run/com.macntfs.mount-active")
                .map_err(|e| e.to_string())?;
            if active.metadata().map_err(|e| e.to_string())?.uid() != 0 {
                return Err("后台挂载锁权限不安全".into());
            }
            if let Ok(meta) = fs::metadata("/var/run/com.macntfs.updating")
                && meta
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age < Duration::from_secs(300))
            {
                return Err("组件正在更新，请稍后重新连接磁盘".into());
            }
            privileged::execute(vec!["mount".into(), req.id, req.uuid, req.backend])
        }
        _ => Err("后台助手仅允许状态检查、安全挂载、推出和更新检查".into()),
    }
}
pub fn serve() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("后台助手必须由系统以 root 启动".into());
    }
    privileged::verify_runtime(Path::new(RUNTIME))?;
    if let Ok(meta) = fs::symlink_metadata(SOCKET) {
        if meta.uid() != 0 || !meta.file_type().is_socket() {
            return Err("后台连接路径不安全".into());
        }
        fs::remove_file(SOCKET).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(SOCKET).map_err(|e| e.to_string())?;
    fs::set_permissions(SOCKET, fs::Permissions::from_mode(0o666)).map_err(|e| e.to_string())?;
    let lock = Arc::new(Mutex::new(()));
    let count = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        let Ok(mut stream) = incoming else { continue };
        if count.fetch_add(1, Ordering::SeqCst) >= 4 {
            count.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let lock = lock.clone();
        let count = count.clone();
        std::thread::spawn(move || {
            let result = (|| {
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .map_err(|e| e.to_string())?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .map_err(|e| e.to_string())?;
                authorize(&stream)?;
                let req = serde_json::from_slice(&read_message(&mut stream)?)
                    .map_err(|e| e.to_string())?;
                dispatch(req, &lock)
            })();
            if let Ok(mut bytes) = serde_json::to_vec(&response(result)) {
                bytes.push(b'\n');
                let _ = stream.write_all(&bytes);
            }
            count.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daemon_rejects_non_mount_operations() {
        for action in ["format", "execute", "install", "status"] {
            let req = Request {
                action: action.into(),
                id: "/etc/passwd".into(),
                uuid: "bad".into(),
                backend: "sh".into(),
            };
            assert!(dispatch(req, &Mutex::new(())).is_err());
        }
        assert!(
            serde_json::from_str::<Request>(
                r#"{"action":"status","id":"","uuid":"","backend":"","command":"id"}"#
            )
            .is_err()
        );
    }
    #[test]
    fn service_bounds_unterminated_requests() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        b.write_all(&[b'x'; 4096]).unwrap();
        assert!(read_message(&mut a).is_err());
    }
}
