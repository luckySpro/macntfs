//! Small, on-demand root helper installed by PKG. No shell, arbitrary path, format,
//! repair or force-unmount operation is exposed. The GUI never launches its own
//! user-writable payload as root.
use crate::system::{self, RUNTIME, Result, Volume};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn check_root_path(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err("组件路径必须为绝对路径".into());
    }
    for component in path.ancestors() {
        let meta = fs::symlink_metadata(component)
            .map_err(|_| format!("组件尚未安装：{}", component.display()))?;
        if meta.file_type().is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err(format!(
                "组件权限不安全，请重新安装：{}",
                component.display()
            ));
        }
    }
    Ok(())
}
// Client bundle descendants must be immutable to ordinary users. /Applications
// itself is root-owned and admin-writable on macOS; validate it separately.
pub(crate) fn check_root_path_until_app(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Err("已安装应用权限不安全，请使用 PKG 重新安装".into());
    }
    let applications = fs::symlink_metadata("/Applications").map_err(|e| e.to_string())?;
    if applications.file_type().is_symlink()
        || applications.uid() != 0
        || applications.mode() & 0o002 != 0
    {
        return Err("应用程序目录权限不安全".into());
    }
    Ok(())
}
pub fn verify_payload(root: &Path) -> Result<()> {
    let manifest = fs::read_to_string(root.join("SHA256SUMS")).map_err(|_| "缺少组件完整性清单")?;
    let mut seen = Vec::new();
    for line in manifest.lines() {
        let (digest, file) = line.split_once("  ").ok_or("组件清单格式无效")?;
        if ![
            "bin/ntfs-3g",
            "bin/ntfs-3g.probe",
            "bin/ntfs-helper",
            "lib/libfuse.2.dylib",
            "MicroVM/SHA256SUMS",
        ]
        .contains(&file)
            || seen.contains(&file)
        {
            return Err("组件清单包含无效或重复路径".into());
        }
        let data = fs::read(root.join(file)).map_err(|_| format!("组件缺失：{file}"))?;
        let actual = format!("{:x}", Sha256::digest(data));
        if digest != actual {
            return Err(format!("组件校验失败：{file}"));
        }
        seen.push(file);
    }
    if seen.len() != 5 {
        return Err("组件清单不完整".into());
    }
    Ok(())
}
pub fn verify_runtime(root: &Path) -> Result<()> {
    for file in [
        "VERSION",
        "SHA256SUMS",
        "bin/ntfs-3g",
        "bin/ntfs-3g.probe",
        "bin/ntfs-helper",
        "lib/libfuse.2.dylib",
        "MicroVM/SHA256SUMS",
    ] {
        check_root_path(&root.join(file))?;
    }
    verify_payload(root)
}
fn command(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "{}（状态 {}）\n{}{}",
            Path::new(program)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
pub(crate) fn validate_volume(id: &str, uuid: &str) -> Result<Volume> {
    if !system::valid_id(id)
        || uuid.len() != 36
        || !uuid.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
    {
        return Err("无效磁盘身份".into());
    }
    let volume = system::scan()?
        .into_iter()
        .find(|v| v.id == id && v.uuid == uuid)
        .ok_or("磁盘已拔出或身份发生变化，请刷新")?;
    Ok(volume)
}
fn restore(volume: &Volume, created: bool, target: &Path) -> String {
    if created {
        let _ = fs::remove_dir(target);
    }
    if !volume.mount.is_empty() && validate_volume(&volume.id, &volume.uuid).is_err() {
        return "\n磁盘已移除或身份变化，未尝试恢复挂载。".into();
    }
    if !volume.mount.is_empty()
        && let Err(error) = command("/usr/sbin/diskutil", &["mount", &volume.id])
    {
        return format!("\n恢复原挂载失败，请重新连接磁盘：{error}");
    }
    String::new()
}
pub fn execute(args: Vec<String>) -> Result<String> {
    if args.len() != 4
        || args[0] != "mount"
        || !["kernel", "fskit", "microvm"].contains(&args[3].as_str())
    {
        return Err("仅接受 mount <partition> <UUID> <kernel|fskit>".into());
    }
    if command("/usr/bin/id", &["-u"])?.trim() != "0" {
        return Err("此操作需要 macOS 管理员授权".into());
    }
    let root = Path::new(RUNTIME);
    verify_runtime(root)?;

    let volume = validate_volume(&args[1], &args[2])?;
    if volume.writable && !volume.mount.is_empty() {
        crate::sessions::validate_mounted_backend(&volume, &args[3], &crate::sessions::load()?)?;
        return Ok("磁盘已经可以读写".into());
    }
    let os = system::environment().os;
    if args[3] == "fskit" && !crate::settings::supports_fskit(&os) {
        return Err("当前 macOS 不支持 FSKit".into());
    }
    let uid = command("/usr/bin/stat", &["-f", "%u", "/dev/console"])?
        .trim()
        .to_owned();
    let gid = command("/usr/bin/stat", &["-f", "%g", "/dev/console"])?
        .trim()
        .to_owned();
    if uid == "0"
        || !uid.bytes().all(|c| c.is_ascii_digit())
        || !gid.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("未找到有效的桌面登录用户".into());
    }
    if args[3] == "microvm" {
        if !crate::microvm::supported(&os) {
            return Err("微虚拟机需要 Apple Silicon 和 macOS 13 或更新版本".into());
        }
        return crate::microvm::mount(&volume, &uid, &gid);
    }
    check_root_path(&root.join("lib/libfuse.2.dylib"))?;
    check_root_path(Path::new("/Library/Filesystems/macfuse.fs"))?;
    let target = PathBuf::from(format!("/Volumes/NTFS-{}", volume.id));
    if fs::symlink_metadata(&target).is_ok() {
        // A previous successful session may leave an empty root-owned mount directory.
        check_root_path(&target)?;
        fs::remove_dir(&target).map_err(|_| "挂载目录被占用，请先关闭其他挂载程序")?;
    }
    if !volume.mount.is_empty() {
        command("/usr/sbin/diskutil", &["unmount", &volume.id])?;
    }
    let mut created = false;
    let mut inflight = false;
    let attempt = (|| -> Result<()> {
        validate_volume(&args[1], &args[2])?;
        let device = format!("/dev/{}", volume.id);
        let probe = Command::new(format!("{RUNTIME}/bin/ntfs-3g.probe"))
            .args(["--readwrite", &device])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| format!("无法启动磁盘检查：{e}"))?;
        if !probe.status.success() {
            return Err(format!(
                "{}\n探测代码：{}\n{}",
                probe_diagnostic(probe.status.code(), &String::from_utf8_lossy(&probe.stderr)),
                probe
                    .status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "进程被终止".into()),
                String::from_utf8_lossy(&probe.stderr)
            ));
        }
        fs::create_dir(&target).map_err(|e| e.to_string())?;
        created = true;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        let mode = if args[3] == "fskit" {
            ",backend=fskit"
        } else {
            ""
        };
        let finder = finder_options(&volume.name);
        let mut options =
            format!("rw,norecover,local,windows_names,uid={uid},gid={gid},{finder}{mode}");
        if args[3] != "fskit" {
            // Larger requests reduce kernel/userspace round trips during copies.
            // Keep FSKit on its own defaults and retain NTFS safety checks.
            options.push_str(",big_writes,iosize=1048576");
        }
        let icon = Path::new(
            "/System/Library/Extensions/IOStorageFamily.kext/Contents/Resources/External.icns",
        );
        if icon.is_file() {
            check_root_path(icon)?;
            options.push_str(&format!(",volicon={}", icon.display()));
        }
        let log_path = root.join("mount.log");
        if log_path.exists() {
            check_root_path(&log_path)?;
        }
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?;
        // Keep the exact child PID until the mount is verified. ntfs-3g otherwise
        // daemonizes before an asynchronous FSKit request has completed.
        let options = format!("{options},no_detach");
        let mut child = Command::new(format!("{RUNTIME}/bin/ntfs-3g"))
            .arg(&device)
            .arg(&target)
            .args(["-o", &options])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| e.to_string())?;
        inflight = true;
        let started = Instant::now();
        loop {
            let current = validate_volume(&args[1], &args[2]);
            if current
                .as_ref()
                .is_ok_and(|v| v.mount == target.to_string_lossy() && v.writable)
            {
                break;
            }
            let exited = child.try_wait().map_err(|e| e.to_string())?;
            if exited.is_some() || started.elapsed() > Duration::from_secs(25) || current.is_err() {
                // Stop only our own foreground process before restoring the old
                // mount. A known mounted session must be unmounted normally.
                let mounted = current
                    .as_ref()
                    .is_ok_and(|v| v.mount == target.to_string_lossy());
                if mounted {
                    command("/usr/sbin/diskutil", &["unmount", &volume.id])?;
                }
                if exited.is_none() {
                    // Ask the driver to unwind and flush. Never SIGKILL a disk
                    // writer or remount over a process still holding the device.
                    let _ = command("/bin/kill", &["-TERM", &child.id().to_string()]);
                    for _ in 0..20 {
                        if child.try_wait().ok().flatten().is_some() {
                            inflight = false;
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                } else {
                    inflight = false;
                }
                let diagnostic = fs::read_to_string(&log_path).unwrap_or_default();
                let lines: Vec<&str> = diagnostic.lines().rev().take(10).collect();
                return Err(format!(
                    "尚未确认读写挂载。请按系统提示批准 macFUSE，再重试；FSKit 失败时可手动选择兼容模式。\n{}",
                    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    })();
    match attempt {
        Ok(()) => {
            crate::sessions::record(&volume, &args[3])?;
            Ok(format!(
                "已开启读写：{} · {}",
                volume.name,
                if args[3] == "fskit" {
                    "FSKit"
                } else {
                    "内核后端"
                }
            ))
        }
        Err(error) => {
            if inflight {
                return Err(format!(
                    "{error}\n驱动仍在等待系统响应，未强行结束或恢复挂载。请完成 macFUSE 授权；若仍未恢复，请重启后再试。"
                ));
            }
            // A daemon may have mounted successfully even if verification failed.
            // Do not replace or unmount any established writable session here.
            let mounted = validate_volume(&args[1], &args[2])
                .ok()
                .is_some_and(|v| v.mount == target.to_string_lossy());
            Err(if mounted {
                error
            } else {
                format!("{error}{}", restore(&volume, created, &target))
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_rejects_arbitrary_commands() {
        for args in [vec!["format"], vec!["mount", "disk4s1", "uuid", "$(id)"]] {
            assert!(execute(args.into_iter().map(String::from).collect()).is_err());
        }
    }
    #[test]
    fn payload_rejects_path_traversal() {
        let dir = std::env::temp_dir().join(format!("ntfs-manifest-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SHA256SUMS"), "00  ../driver").unwrap();
        assert!(verify_payload(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}

/// EPERM from opening a macOS device is not a filesystem hibernation diagnosis.
/// Support older installed probes as well as our patched driver.
pub fn probe_diagnostic(code: Option<i32>, diagnostic: &str) -> &'static str {
    if diagnostic.lines().any(|line| {
        line.starts_with("Error opening '")
            && (line.contains("Operation not permitted") || line.contains("Permission denied"))
    }) {
        return probe_message(Some(19));
    }
    probe_message(code)
}

pub fn probe_message(code: Option<i32>) -> &'static str {
    match code {
        Some(12) => "磁盘未识别为 NTFS，请刷新磁盘列表。",
        Some(13) => "NTFS 文件系统异常。请在 Windows 中检查磁盘后重试。",
        Some(14) => "检测到 Windows 休眠或缓存状态。请关闭快速启动并完整关机后重试。",
        Some(15) => "磁盘未正常推出。请在 Windows 检查磁盘并安全推出后重试。",
        Some(16) => "磁盘被其他程序占用。请退出 Mounty 等挂载工具并重新连接磁盘。",
        Some(19) => {
            "macOS 拒绝访问磁盘设备。请在「完整磁盘访问」中重新添加并授权权限助手 ntfs-helper，然后重试。"
        }
        Some(21) => "文件系统驱动尚未就绪。请检查 macFUSE 安装和系统授权。",
        Some(22) => "磁盘或组件权限不安全，已停止开启读写。",
        _ => "磁盘检查未完成，请展开技术详情查看原因。",
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn classifies_probe_failures_without_assuming_corruption() {
        assert!(probe_message(Some(16)).contains("占用"));
        assert!(!probe_message(Some(19)).contains("Windows"));
        assert!(probe_message(Some(14)).contains("休眠"));
        assert!(probe_message(Some(13)).contains("检查磁盘"));
        assert!(probe_message(None).contains("技术详情"));
    }
}

#[cfg(test)]
mod macos_permission_tests {
    use super::*;
    #[test]
    fn device_open_eperm_is_not_windows_hibernation() {
        let diagnostic = "Error opening '/dev/disk4s3': Operation not permitted\n";
        assert!(probe_diagnostic(Some(14), diagnostic).contains("macOS"));
        assert!(!probe_diagnostic(Some(14), diagnostic).contains("休眠"));
        assert!(probe_diagnostic(Some(19), diagnostic).contains("授权"));
    }
    #[test]
    fn real_hibernation_stays_blocked() {
        for diagnostic in [
            "Windows is hibernated, refused to mount.",
            "Metadata kept in Windows cache, refused to mount.",
        ] {
            assert!(probe_diagnostic(Some(14), diagnostic).contains("休眠"));
        }
    }
}

/// macFUSE options are comma-separated, so labels must never inject options.
pub fn finder_options(name: &str) -> String {
    let mut label = String::new();
    for c in name.chars() {
        let c = match c {
            ',' => '，',
            '\\' => '＼',
            c if c.is_control() => ' ',
            c => c,
        };
        if label.len() + c.len_utf8() > 255 {
            break;
        }
        label.push(c);
    }
    let label = label.trim();
    format!(
        "streams_interface=openxattr,volname={}",
        if label.is_empty() { "NTFS" } else { label }
    )
}
#[cfg(test)]
mod finder_tests {
    use super::*;
    #[test]
    fn finder_label_cannot_inject_mount_flags() {
        assert_eq!(
            finder_options("BackUp"),
            "streams_interface=openxattr,volname=BackUp"
        );
        assert_eq!(
            finder_options(""),
            "streams_interface=openxattr,volname=NTFS"
        );
        assert_eq!(
            finder_options("Data,allow_other\\test\n"),
            "streams_interface=openxattr,volname=Data，allow_other＼test"
        );
        assert_eq!(
            finder_options(&"盘".repeat(100))
                .split("volname=")
                .nth(1)
                .unwrap()
                .len(),
            255
        );
    }
}

pub(crate) fn probe_safe(id: &str) -> Result<()> {
    if !system::valid_id(id) {
        return Err("无效磁盘身份".into());
    }
    let out = Command::new(format!("{RUNTIME}/bin/ntfs-3g.probe"))
        .args(["--readwrite", &format!("/dev/{id}")])
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(probe_diagnostic(out.status.code(), &String::from_utf8_lossy(&out.stderr)).into())
    }
}
pub(crate) fn eject(id: &str, uuid: &str) -> Result<String> {
    let volume = validate_volume(id, uuid)?;
    let sessions = crate::sessions::load()?;
    // Ejecting a physical disk includes every managed VM partition on that disk.
    let physical = system::scan_physical()?;
    for session in sessions.iter().filter(|s| s.backend == "microvm") {
        if physical
            .iter()
            .any(|v| v.id == session.id && v.uuid == session.uuid && v.parent == volume.parent)
        {
            crate::microvm::command(&["unmount", &session.target, "--wait-for-vm", "30"], 40)?;
            if crate::sessions::mount_table()?
                .iter()
                .any(|m| m.target == session.target)
            {
                return Err("磁盘仍在使用，已停止推出。请关闭文件后重试".into());
            }
            // Remove only our recorded, root-owned empty mount point after the
            // VM has released it. Never remove contents or reuse another volume.
            let target = Path::new(&session.target);
            if target.exists() && check_root_path(target).is_ok() {
                let _ = fs::remove_dir(target);
            }
        }
    }
    if !volume
        .parent
        .strip_prefix("disk")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err("无效磁盘标识".into());
    }
    command("/usr/sbin/diskutil", &["eject", &volume.parent])?;
    Ok("已安全推出整块磁盘，可拔下连接线".into())
}
pub fn ensure_update_safe() -> Result<()> {
    crate::sessions::load()?;
    let mounts = crate::sessions::mount_table()?;
    if !crate::sessions::managed_mounts(&mounts).is_empty() {
        return Err("更新前请先安全推出 macntfs 管理的磁盘；关闭正在使用的文件后重试".into());
    }
    // Also reject a VM/driver still starting or tearing down without a mount entry.
    for executable in [
        Path::new(RUNTIME).join("bin/ntfs-3g"),
        crate::microvm::root().join("bin/anylinuxfs"),
    ] {
        if executable.exists() {
            let out = Command::new("/usr/sbin/lsof")
                .args(["-t", "--"])
                .arg(executable)
                .output()
                .map_err(|e| e.to_string())?;
            match out.status.code() {
                Some(1) => {}
                Some(0) => return Err("读写服务尚未退出，请稍后重试更新".into()),
                _ => return Err("无法确认读写服务已退出，已停止更新".into()),
            }
        }
    }
    Ok(())
}
