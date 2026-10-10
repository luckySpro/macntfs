//! Opt-in, offline anylinuxfs/libkrun backend. Only fixed root-owned paths are executed.
use crate::{
    privileged, sessions,
    system::{self, RUNTIME, Result, Volume},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn root() -> PathBuf {
    Path::new(RUNTIME).join("MicroVM")
}
pub fn supported(os: &str) -> bool {
    cfg!(target_arch = "aarch64")
        && os
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .is_some_and(|v| v >= 13)
}
pub fn ready() -> bool {
    root().join("bin/anylinuxfs").is_file()
        && root().join("profile/alpine/rootfs.ver").is_file()
        && root().join("SHA256SUMS").is_file()
}
pub fn verify_manifest(root: &Path, require_root: bool) -> Result<()> {
    if require_root {
        privileged::check_root_path(&root.join("SHA256SUMS"))?;
    }
    let manifest = fs::read_to_string(root.join("SHA256SUMS"))
        .map_err(|_| "微虚拟机离线组件未安装，请安装完整 PKG")?;
    let mut seen = std::collections::HashSet::new();
    for line in manifest.lines() {
        let (digest, entry) = line.split_once("  ").ok_or("微虚拟机组件清单无效")?;
        let (kind, name) = entry.split_once(' ').ok_or("微虚拟机组件清单无效")?;
        if !["F", "L"].contains(&kind)
            || !Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            || !seen.insert(name)
        {
            return Err("微虚拟机组件清单无效".into());
        }
        let path = root.join(name);
        let meta = fs::symlink_metadata(&path).map_err(|_| "微虚拟机离线组件缺失")?;
        let bytes = if kind == "L" {
            if !meta.file_type().is_symlink() {
                return Err("微虚拟机链接校验失败".into());
            }
            if require_root {
                privileged::check_root_path(path.parent().ok_or("无效组件路径")?)?;
                if meta.uid() != 0 {
                    return Err("微虚拟机权限不安全".into());
                }
            }
            fs::read_link(&path)
                .map_err(|e| e.to_string())?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err("微虚拟机文件校验失败".into());
            }
            if require_root {
                privileged::check_root_path(&path)?;
            }
            fs::read(&path).map_err(|e| e.to_string())?
        };
        if format!("{:x}", Sha256::digest(&bytes)) != digest {
            return Err("微虚拟机组件完整性校验失败，请重新安装完整 PKG".into());
        }
    }
    for file in [
        "bin/anylinuxfs",
        "libexec/gvproxy",
        "libexec/Image",
        "libexec/vmproxy",
        "profile/alpine/rootfs.ver",
        "profile/alpine/rootfs/vmproxy",
        "profile/alpine/rootfs/usr/local/bin/entrypoint.sh",
        "etc/anylinuxfs.toml",
    ] {
        if !seen.contains(file) {
            return Err("微虚拟机组件清单不完整".into());
        }
    }
    Ok(())
}
pub fn command(args: &[&str], seconds: u64) -> Result<String> {
    privileged::check_root_path(&root().join("bin/anylinuxfs"))?;
    let log = root().join("operation.log");
    if log.exists() {
        privileged::check_root_path(&log)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| e.to_string())?;
    let log_start = file.metadata().map_err(|e| e.to_string())?.len();
    // launchd has no sudo ancestry. Resolve the desktop identity from the OS,
    // never from client arguments or inherited environment variables.
    let console = fs::symlink_metadata("/dev/console").map_err(|e| e.to_string())?;
    if console.file_type().is_symlink() {
        return Err("未找到有效的桌面登录用户".into());
    }
    let mut process = Command::new(root().join("bin/anylinuxfs"));
    configure_invoker(&mut process, console.uid(), console.gid())?;
    let mut child = process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(file))
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok("微虚拟机操作完成".into());
            }
            return Err(format!(
                "微虚拟机操作失败。未强行恢复或卸载磁盘。\n退出状态：{status}\n{}",
                operation_detail(&log, log_start).unwrap_or_default()
            ));
        }
        if started.elapsed() > Duration::from_secs(seconds) {
            return Err(
                "微虚拟机仍在等待系统响应。未强行结束磁盘读写；请运行诊断后安全推出。".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn operation_detail(path: &Path, start: u64) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let end = file.metadata()?.len();
    file.seek(SeekFrom::Start(start.max(end.saturating_sub(8192))))?;
    let mut bytes = Vec::new();
    file.take(8192).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).trim().into())
}
fn configure_invoker(process: &mut Command, uid: u32, gid: u32) -> Result<()> {
    if uid == 0 {
        return Err("未找到有效的桌面登录用户".into());
    }
    process
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        // anylinuxfs consumes these as identity metadata, not authentication.
        .env("SUDO_UID", uid.to_string())
        .env("SUDO_GID", gid.to_string());
    Ok(())
}
pub fn mount(volume: &Volume, uid: &str, gid: &str) -> Result<String> {
    verify_manifest(&root(), true)?;
    let target = sessions::target(&volume.id, "microvm");
    if Path::new(&target).exists() {
        privileged::check_root_path(Path::new(&target))?;
        fs::remove_dir(&target).map_err(|_| "挂载目录被占用，请先安全推出")?;
    }
    if !volume.mount.is_empty() {
        system::run("/usr/sbin/diskutil", &["unmount", &volume.id])?;
    }
    // Probe after macOS releases the device; never use recover/force/remove_hiberfile.
    if let Err(e) = privileged::probe_safe(&volume.id) {
        if !volume.mount.is_empty() {
            let _ = system::run("/usr/sbin/diskutil", &["mount", &volume.id]);
        }
        return Err(e);
    }
    privileged::validate_volume(&volume.id, &volume.uuid)?;
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let device = format!("/dev/{}", volume.id);
    let options = format!("rw,norecover,windows_names,uid={uid},gid={gid}");
    // Keep the intended session even if startup times out, so safe eject can find it.
    sessions::record(volume, "microvm")?;
    // Explicit loopback binding; no LAN listener, no PF/default-route changes.
    command(
        &[
            "mount",
            &device,
            &target,
            "-t",
            "ntfs-3g",
            "-o",
            &options,
            "--net-helper",
            "gvproxy",
            "--bind-addr",
            "127.0.0.1",
            "--nfs-options",
            "soft,tcp,vers=3,nodev,nosuid",
            "--window",
            "false",
        ],
        90,
    )?;
    let table = sessions::mount_table()?;
    if !table.iter().any(|m| {
        m.target == target
            && m.kind == "nfs"
            && m.writable
            && sessions::is_loopback_source(&m.source)
    }) {
        return Err("尚未确认微虚拟机读写挂载，请运行诊断".into());
    }
    privileged::validate_volume(&volume.id, &volume.uuid)?;
    Ok("已开启读写：微虚拟机实验模式".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launchd_child_receives_only_trusted_non_root_identity() {
        let mut process = Command::new("/bin/sh");
        process.env("SUDO_UID", "0").env("HOME", "/untrusted");
        configure_invoker(&mut process, 501, 20).unwrap();
        let output = process
            .args([
                "-c",
                "printf '%s:%s:%s' \"$SUDO_UID\" \"$SUDO_GID\" \"${HOME-unset}\"",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"501:20:unset");
        assert!(configure_invoker(&mut Command::new("/bin/sh"), 0, 0).is_err());
    }
    #[test]
    fn operation_errors_exclude_old_attempts_and_are_bounded() {
        let path = std::env::temp_dir().join(format!("macntfs-vm-log-{}", std::process::id()));
        fs::write(&path, b"old permission error\nnew startup failure\n").unwrap();
        assert_eq!(operation_detail(&path, 21).unwrap(), "new startup failure");
        fs::write(&path, vec![b'a'; 20000]).unwrap();
        assert_eq!(operation_detail(&path, 0).unwrap().len(), 8192);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn rejects_unsafe_manifest_paths_and_missing_contract() {
        let root = std::env::temp_dir().join(format!("macntfs-vm-manifest-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for manifest in ["00  F ../bin/tool", "00  F /etc/passwd", "00  X tool", ""] {
            fs::write(root.join("SHA256SUMS"), manifest).unwrap();
            assert!(verify_manifest(&root, false).is_err());
        }
        fs::remove_dir_all(root).unwrap();
        assert!(!supported("12.7"));
    }
}
