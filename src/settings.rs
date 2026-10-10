use crate::system::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    #[default]
    Auto,
    Fskit,
    Kernel,
    Microvm,
}
impl Backend {
    pub fn title(self) -> &'static str {
        match self {
            Self::Auto => "稳定模式（推荐）",
            Self::Fskit => "FSKit · 实验性",
            Self::Kernel => "内核 · 兼容模式",
            Self::Microvm => "微虚拟机 · 实验性",
        }
    }
    pub fn resolved(self, os: &str) -> Result<&'static str> {
        let supported = supports_fskit(os);
        match self {
            Self::Auto => Ok("kernel"),
            Self::Fskit if supported => Ok("fskit"),
            Self::Fskit => Err("FSKit 需要 macOS 15.4 或更新版本".into()),
            Self::Kernel => Ok("kernel"),
            Self::Microvm if crate::microvm::supported(os) => Ok("microvm"),
            Self::Microvm => Err("微虚拟机需要 Apple Silicon 和 macOS 13 或更新版本".into()),
        }
    }
}
pub fn supports_fskit(os: &str) -> bool {
    let parts: Vec<u32> = os
        .trim()
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    match parts.as_slice() {
        [major, ..] if *major > 15 => true,
        [15, minor, ..] if *minor >= 4 => true,
        _ => false,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Stone,
    Office,
    Graphite,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub backend: Backend,
    pub dark: bool,
    pub theme: Theme,
    pub auto_mount: bool,
    pub language: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Auto,
            dark: false,
            theme: Theme::Stone,
            auto_mount: true,
            language: "auto".into(),
        }
    }
}
pub fn data_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("MACNTFS_TEST_DATA_DIR") {
        return PathBuf::from(path);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/NTFS Desktop")
}
impl Settings {
    pub fn load() -> Self {
        fs::read(data_dir().join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| {
                let os = crate::system::run("/usr/bin/sw_vers", &["-productVersion"])
                    .unwrap_or_default();
                Self {
                    backend: initial_backend(
                        &os,
                        std::path::Path::new("/Library/Filesystems/macfuse.fs").is_dir(),
                    ),
                    ..Self::default()
                }
            })
    }
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
        let path = data_dir().join("settings.tmp");
        fs::write(
            &path,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(path, data_dir().join("settings.json")).map_err(|e| e.to_string())
    }
}
pub fn initial_backend(os: &str, fuse_installed: bool) -> Backend {
    if !fuse_installed && crate::microvm::supported(os) {
        Backend::Microvm
    } else {
        Backend::Auto
    }
}
pub fn validate_backend_change(
    previous: Backend,
    next: Backend,
    os: &str,
    busy: bool,
    active_mounts: bool,
) -> Result<()> {
    if previous == next {
        return Ok(());
    }
    next.resolved(os)?;
    if busy || active_mounts {
        return Err(
            "请先安全推出所有受管理磁盘，再切换读写模式。重新连接后使用所选模式，不会自动回退。"
                .into(),
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_settings_keep_backend_and_automount() {
        let settings: Settings =
            serde_json::from_str(r#"{"backend":"Kernel","dark":false,"auto_mount":false}"#)
                .unwrap();
        assert_eq!(settings.language, "auto");
        assert_eq!(settings.theme, Theme::Stone);
        assert_eq!(settings.backend, Backend::Kernel);
        assert!(!settings.auto_mount);
        for theme in [Theme::Stone, Theme::Office, Theme::Graphite] {
            let settings = Settings {
                theme,
                ..Settings::default()
            };
            let restored: Settings =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            assert_eq!(restored.theme, theme);
        }
    }
    #[test]
    fn selects_supported_backends() {
        assert!(!supports_fskit("14.7"));
        assert!(!supports_fskit("15.3.2"));
        assert!(supports_fskit("15.4"));
        assert!(supports_fskit("26.0"));
        assert_eq!(Backend::Auto.resolved("15.3").unwrap(), "kernel");
        assert!(Backend::Fskit.resolved("14.7").is_err());
    }
    #[test]
    fn switching_requires_idle_unmounted_disks_but_allows_other_preferences() {
        assert!(
            validate_backend_change(Backend::Auto, Backend::Microvm, "27.0", false, true).is_err()
        );
        assert!(
            validate_backend_change(Backend::Auto, Backend::Kernel, "27.0", true, false).is_err()
        );
        assert!(validate_backend_change(Backend::Auto, Backend::Auto, "27.0", true, true).is_ok());
        assert!(
            validate_backend_change(Backend::Microvm, Backend::Kernel, "27.0", false, false)
                .is_ok()
        );
        assert!(
            validate_backend_change(Backend::Auto, Backend::Fskit, "14.0", false, false).is_err()
        );
        assert_eq!(initial_backend("12.0", false), Backend::Auto);
        assert_eq!(initial_backend("27.0", true), Backend::Auto);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(initial_backend("27.0", false), Backend::Microvm);
    }
}
