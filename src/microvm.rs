//! Opt-in, offline anylinuxfs/libkrun backend. Only fixed root-owned paths are executed.
use crate::{
    privileged, sessions,
    system::{self, RUNTIME, Result, Volume},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
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
    let mut child = Command::new(root().join("bin/anylinuxfs"))
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
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
            return Err(
                "微虚拟机操作失败。请检查磁盘访问权限并运行诊断；未强行恢复或卸载磁盘。".into(),
            );
        }
        if started.elapsed() > Duration::from_secs(seconds) {
            return Err(
                "微虚拟机仍在等待系统响应。未强行结束磁盘读写；请运行诊断后安全推出。".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
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
