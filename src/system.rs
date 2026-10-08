use plist::{Dictionary, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Volume {
    pub id: String,
    pub parent: String,
    pub uuid: String,
    pub name: String,
    pub mount: String,
    pub size: u64,
    pub free: u64,
    pub writable: bool,
}
pub const RUNTIME: &str = "/Library/Application Support/NTFS Desktop/Runtime";
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Environment {
    pub runtime: bool,
    pub bundled: bool,
    pub fuse: bool,
    pub fuse_version: String,
    pub os: String,
    pub runtime_issue: String,
    pub service: bool,
}
pub fn resources() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let contents = executable.parent()?.parent()?;
    let path = contents.join("Resources");
    path.is_dir().then_some(path)
}
pub fn environment() -> Environment {
    let check = crate::privileged::verify_runtime(Path::new(RUNTIME)).and_then(|_| {
        let installed =
            std::fs::read_to_string(Path::new(RUNTIME).join("VERSION")).unwrap_or_default();
        if installed.trim() == env!("CARGO_PKG_VERSION") {
            Ok(())
        } else {
            Err("读写组件版本需要更新，请点击安装组件".into())
        }
    });
    let version = Value::from_file("/Library/Filesystems/macfuse.fs/Contents/version.plist")
        .ok()
        .and_then(|v| {
            v.as_dictionary()
                .map(|d| string(d, "CFBundleShortVersionString"))
        })
        .unwrap_or_default();
    Environment {
        runtime: check.is_ok(),
        bundled: resources().is_some_and(|r| {
            r.join("Installers/OfflineRuntime.pkg").is_file()
                && r.join("Installers/Install macFUSE.pkg").is_file()
        }),
        fuse: Path::new("/Library/Filesystems/macfuse.fs").is_dir(),
        fuse_version: version,
        os: run("/usr/bin/sw_vers", &["-productVersion"])
            .unwrap_or_default()
            .trim()
            .into(),
        runtime_issue: check.err().unwrap_or_default(),
        service: crate::daemon::ready(),
    }
}
pub fn run(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        )
        .trim()
        .to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn read_plist(args: &[&str]) -> Result<Value> {
    let out = Command::new("/usr/sbin/diskutil")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    plist::from_bytes(&out.stdout).map_err(|e| e.to_string())
}
fn string(d: &Dictionary, key: &str) -> String {
    d.get(key)
        .and_then(Value::as_string)
        .unwrap_or_default()
        .into()
}
fn boolean(d: &Dictionary, key: &str) -> bool {
    d.get(key).and_then(Value::as_boolean).unwrap_or(false)
}
fn number(d: &Dictionary, key: &str) -> u64 {
    d.get(key).and_then(Value::as_unsigned_integer).unwrap_or(0)
}
pub fn valid_id(id: &str) -> bool {
    let Some(suffix) = id.strip_prefix("disk") else {
        return false;
    };
    let Some((disk, slice)) = suffix.split_once('s') else {
        return false;
    };
    !disk.is_empty()
        && !slice.is_empty()
        && disk.bytes().all(|c| c.is_ascii_digit())
        && slice.bytes().all(|c| c.is_ascii_digit())
}
fn parse_volume(value: &Value) -> Result<Volume> {
    let d = value.as_dictionary().ok_or("磁盘信息格式无效")?;
    let id = string(d, "DeviceIdentifier");
    if !valid_id(&id)
        || boolean(d, "Internal")
        || boolean(d, "WholeDisk")
        || boolean(d, "SystemImage")
        || !boolean(d, "WritableMedia")
    {
        return Err("仅支持可写的外置物理磁盘分区".into());
    }
    if ![
        "FilesystemType",
        "FilesystemName",
        "FilesystemUserVisibleName",
    ]
    .iter()
    .any(|k| string(d, k).to_lowercase().contains("ntfs"))
    {
        return Err("该分区不是 NTFS".into());
    }
    Ok(Volume {
        id,
        parent: string(d, "ParentWholeDisk"),
        uuid: string(d, "VolumeUUID"),
        name: string(d, "VolumeName"),
        mount: string(d, "MountPoint"),
        size: number(d, "TotalSize"),
        free: number(d, "FreeSpace"),
        writable: boolean(d, "WritableVolume"),
    })
}
pub fn scan() -> Result<Vec<Volume>> {
    let list = read_plist(&["list", "-plist", "external", "physical"])?;
    let ids = list
        .as_dictionary()
        .and_then(|d| d.get("AllDisks"))
        .and_then(Value::as_array)
        .ok_or("无法读取外置磁盘列表")?;
    let mut volumes = Vec::new();
    for id in ids
        .iter()
        .filter_map(Value::as_string)
        .filter(|id| valid_id(id))
    {
        if let Ok(info) = read_plist(&["info", "-plist", id])
            && let Ok(volume) = parse_volume(&info)
        {
            volumes.push(volume);
        }
    }
    Ok(volumes)
}
fn fresh(v: &Volume) -> Result<Volume> {
    let current = scan()?
        .into_iter()
        .find(|x| x.id == v.id)
        .ok_or("磁盘已移除，请刷新")?;
    if current.uuid.is_empty() || current.uuid != v.uuid {
        return Err("磁盘身份发生变化或缺少 UUID，请刷新后重试".into());
    }
    Ok(current)
}
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
pub fn mount(v: &Volume, backend: crate::settings::Backend) -> Result<String> {
    let v = fresh(v)?;
    if v.writable && !v.mount.is_empty() {
        return Ok("磁盘已经可以读写".into());
    }
    let env = environment();
    if !env.runtime {
        return Err(env.runtime_issue);
    }
    if !env.fuse {
        return Err("请先完成离线驱动安装".into());
    }
    crate::privileged::verify_runtime(Path::new(RUNTIME))?;
    let mode = backend.resolved(&env.os)?;
    crate::daemon::request("mount", &v.id, &v.uuid, mode)
}

pub fn eject(v: &Volume) -> Result<String> {
    let v = fresh(v)?;
    let parent = v
        .parent
        .strip_prefix("disk")
        .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        .ok_or("无效磁盘标识")?;
    run("/usr/sbin/diskutil", &["eject", &format!("disk{parent}")])?;
    Ok("已安全推出整块磁盘，可拔下连接线".into())
}
pub fn open_volume(v: &Volume) -> Result<String> {
    let v = fresh(v)?;
    if v.mount.is_empty() {
        return Err("磁盘尚未挂载".into());
    }
    run("/usr/bin/open", &[&v.mount])?;
    Ok("已在 Finder 中打开".into())
}
pub fn open_settings() -> Result<String> {
    run(
        "/usr/bin/open",
        &["x-apple.systempreferences:com.apple.preference.security?Privacy"],
    )
}
pub fn open_disk_permissions() -> Result<String> {
    run(
        "/usr/bin/open",
        &["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"],
    )?;
    Ok("已打开完整磁盘访问设置。请使用「打开助手位置」，将 ntfs-helper 添加到列表并开启；更新助手后若权限失效，请移除旧条目再重新添加。".into())
}

pub fn reveal_permission_helper() -> Result<String> {
    let path = Path::new(RUNTIME).join("bin/ntfs-helper");
    if !path.is_file() {
        return Err("请先安装内置读写组件".into());
    }
    run("/usr/bin/open", &["-R", &path.to_string_lossy()])?;
    Ok("已在 Finder 选中 ntfs-helper。请将它添加到「完整磁盘访问」列表并开启授权。".into())
}

pub fn open_release() -> Result<String> {
    run(
        "/usr/bin/open",
        &["https://github.com/luckySpro/macntfs/releases/latest"],
    )?;
    Ok("已打开 GitHub 完整安装包下载页。下载 PKG 后完成系统安装。".into())
}

pub fn install() -> Result<String> {
    let resources = resources().ok_or("请使用离线安装包，开发运行模式不包含安装资源")?;
    let env = environment();
    let package = if !env.runtime {
        resources.join("Installers/OfflineRuntime.pkg")
    } else if !env.fuse {
        resources.join("Installers/Install macFUSE.pkg")
    } else {
        return Ok("离线运行组件已经安装。若系统仍要求驱动授权，请前往系统设置。".into());
    };
    if !package.is_file() {
        return Err("离线安装资源缺失，请重新安装完整 PKG".into());
    }
    run(
        "/usr/bin/open",
        &["-a", "Installer", &package.to_string_lossy()],
    )?;
    Ok("已打开本地安装器。完成安装后返回应用刷新；全程无需下载。".into())
}
pub fn open_data_dir() -> Result<String> {
    std::fs::create_dir_all(crate::settings::data_dir()).map_err(|e| e.to_string())?;
    run(
        "/usr/bin/open",
        &[&crate::settings::data_dir().to_string_lossy()],
    )
}
pub fn home_font() -> Option<PathBuf> {
    [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_are_restricted() {
        for id in ["disk4s3", "disk12s10"] {
            assert!(valid_id(id));
        }
        for id in [
            "disk4",
            "disk4s3;id",
            "disk4s",
            "diskxs2",
            "disk4s1s2",
            "../disk4s3",
        ] {
            assert!(!valid_id(id));
        }
    }
    #[test]
    fn quotes_are_literal() {
        assert_eq!(shell_quote("a'b $(id)"), "'a'\\''b $(id)'");
    }
    #[test]
    fn rejects_internal_and_non_ntfs() {
        let mut d = Dictionary::new();
        d.insert("DeviceIdentifier".into(), Value::String("disk4s3".into()));
        d.insert("FilesystemType".into(), Value::String("ntfs".into()));
        d.insert("WritableMedia".into(), Value::Boolean(true));
        assert!(parse_volume(&Value::Dictionary(d.clone())).is_ok());
        d.insert("Internal".into(), Value::Boolean(true));
        assert!(parse_volume(&Value::Dictionary(d.clone())).is_err());
        d.insert("Internal".into(), Value::Boolean(false));
        d.insert("FilesystemType".into(), Value::String("exfat".into()));
        assert!(parse_volume(&Value::Dictionary(d)).is_err());
    }
}
