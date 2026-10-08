use crate::system::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    #[default]
    Auto,
    Fskit,
    Kernel,
}
impl Backend {
    pub fn title(self) -> &'static str {
        match self {
            Self::Auto => "稳定模式（推荐）",
            Self::Fskit => "FSKit · 实验性",
            Self::Kernel => "内核 · 兼容模式",
        }
    }
    pub fn resolved(self, os: &str) -> Result<&'static str> {
        let supported = supports_fskit(os);
        match self {
            Self::Auto => Ok("kernel"),
            Self::Fskit if supported => Ok("fskit"),
            Self::Fskit => Err("FSKit 需要 macOS 15.4 或更新版本".into()),
            Self::Kernel => Ok("kernel"),
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
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Auto,
            dark: false,
            theme: Theme::Stone,
            auto_mount: true,
        }
    }
}
pub fn data_dir() -> PathBuf {
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
            .unwrap_or_default()
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_settings_keep_backend_and_automount() {
        let settings: Settings =
            serde_json::from_str(r#"{"backend":"Kernel","dark":false,"auto_mount":false}"#)
                .unwrap();
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
}
