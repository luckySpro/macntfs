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
        "GUEST-METADATA.json",
    ] {
        if !seen.contains(file) {
            return Err("微虚拟机组件清单不完整".into());
        }
    }
    Ok(())
}
/// Restore guest-only permissions after Installer has unpacked the payload.
/// Host files remain root-owned and immutable; no macOS setuid bits are added.
pub fn prepare_guest_metadata() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("配置虚拟机元数据需要安装器授权".into());
    }
    let runtime = root();
    verify_manifest(&runtime, true)?;
    let metadata: std::collections::BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(runtime.join("GUEST-METADATA.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let guest = runtime.join("profile/alpine/rootfs");
    // Validate the whole list before changing any attribute.
    for (name, value) in &metadata {
        validate_guest_metadata(name, value)?;
        privileged::check_root_path(&guest.join(name))?;
    }
    for (name, value) in metadata {
        // Installer preserves existing directory modes during upgrades. Repair
        // host accessibility explicitly, rather than relying on payload modes.
        let path = guest.join(&name);
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(guest_host_mode(meta.mode(), meta.is_dir())),
        )
        .map_err(|e| e.to_string())?;
        let status = Command::new("/usr/bin/xattr")
            .args(["-w", "user.containers.override_stat", &value])
            .arg(guest.join(name))
            .env_clear()
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("虚拟机文件元数据配置失败，请重新安装完整 PKG".into());
        }
    }
    Ok(())
}
fn guest_host_mode(mode: u32, directory: bool) -> u32 {
    if directory {
        0o755
    } else {
        (mode & 0o111) | 0o644
    }
}
fn validate_guest_metadata(name: &str, value: &str) -> Result<()> {
    if name.is_empty()
        || !Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("无效虚拟机元数据路径".into());
    }
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3
        || parts[0].parse::<u32>().is_err()
        || parts[1].parse::<u32>().is_err()
        || parts[2].is_empty()
        || !parts[2].bytes().all(|b| (b'0'..=b'7').contains(&b))
        || u32::from_str_radix(parts[2], 8).is_err()
    {
        return Err("无效虚拟机文件元数据".into());
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
    let target = named_target(&volume.name, &volume.id)?;
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let device = format!("/dev/{}", volume.id);
    let options = format!("rw,norecover,windows_names,uid={uid},gid={gid}");
    // Keep the intended session even if startup times out, so safe eject can find it.
    sessions::record_at(volume, "microvm", &target)?;
    let log = root().join("operation.log");
    let log_start = fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
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
        return Err(format!(
            "尚未确认微虚拟机读写挂载，请运行诊断\n{}",
            operation_detail(&log, log_start).unwrap_or_default()
        ));
    }
    privileged::validate_volume(&volume.id, &volume.uuid)?;
    Ok("已开启读写：微虚拟机实验模式".into())
}
fn named_target(name: &str, id: &str) -> Result<String> {
    choose_named_target(Path::new("/Volumes"), name, id)
}
fn choose_named_target(directory: &Path, name: &str, id: &str) -> Result<String> {
    let label: String = name
        .trim()
        .chars()
        .take(80)
        .map(|c| {
            if c == '/' || c == ':' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let label = if label.is_empty() || label == "." || label == ".." {
        "NTFS"
    } else {
        &label
    };
    for suffix in 0..100 {
        let target = if suffix == 0 {
            directory.join(label).to_string_lossy().into_owned()
        } else {
            directory
                .join(format!("{label}-{id}-{suffix}"))
                .to_string_lossy()
                .into_owned()
        };
        match fs::symlink_metadata(&target) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(target),
            Err(e) => return Err(e.to_string()),
            Ok(_) => {}
        }
    }
    Err("没有可用的磁盘挂载名称".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_labels_are_safe_and_existing_paths_are_not_reused() {
        let directory = std::env::temp_dir().join(format!("macntfs-label-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("BackUp"), b"existing user data").unwrap();
        assert_eq!(
            choose_named_target(&directory, "BackUp", "disk5s3").unwrap(),
            directory.join("BackUp-disk5s3-1").to_string_lossy()
        );
        assert_eq!(
            fs::read(directory.join("BackUp")).unwrap(),
            b"existing user data"
        );
        assert_eq!(
            choose_named_target(&directory, "a/b:c", "disk5s3").unwrap(),
            directory.join("a_b_c").to_string_lossy()
        );
        assert_eq!(
            choose_named_target(&directory, "..", "disk5s3").unwrap(),
            directory.join("NTFS").to_string_lossy()
        );
        fs::remove_dir_all(directory).unwrap();
        assert!(!sessions::valid_volume_target("/Volumes/a/../../etc"));
        assert!(!sessions::valid_volume_target("/Volumes/name\n"));
    }
    #[test]
    fn upgrades_repair_private_host_modes_without_host_setuid_or_shared_writes() {
        assert_eq!(guest_host_mode(0o40700, true), 0o755);
        assert_eq!(guest_host_mode(0o100600, false), 0o644);
        assert_eq!(guest_host_mode(0o104755, false), 0o755);
    }
    #[test]
    fn guest_metadata_rejects_traversal_and_invalid_permissions() {
        assert!(validate_guest_metadata("bin/mount", "0:0:0104755").is_ok());
        assert!(validate_guest_metadata("etc/passwd", "100:100:0644").is_ok());
        for name in ["", "../outside", "/bin/mount", "bin/../mount"] {
            assert!(validate_guest_metadata(name, "0:0:0755").is_err());
        }
        for value in ["0:0", "0:0:888", "0:0:", "a:0:755", "0:0:755:0"] {
            assert!(validate_guest_metadata("bin/mount", value).is_err());
        }
    }
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
