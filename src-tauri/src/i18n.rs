use macntfs_core::settings::Settings;
use std::sync::OnceLock;
static SYSTEM: OnceLock<String> = OnceLock::new();
static MESSAGES: OnceLock<serde_json::Value> = OnceLock::new();
pub fn text(key: &str) -> String {
    let settings = Settings::load();
    let language = if settings.language == "auto" {
        SYSTEM
            .get_or_init(|| {
                let output = std::process::Command::new("/usr/bin/defaults")
                    .args(["read", "-g", "AppleLanguages"])
                    .output()
                    .ok();
                let source = output
                    .map(|v| String::from_utf8_lossy(&v.stdout).into_owned())
                    .unwrap_or_default();
                let first = source
                    .lines()
                    .map(str::trim)
                    .find(|s| !s.is_empty() && *s != "(" && *s != ")")
                    .unwrap_or("en");
                if first.contains("zh") {
                    if first.contains("Hant") || first.contains("TW") || first.contains("HK") {
                        "zh-Hant"
                    } else {
                        "zh-Hans"
                    }
                } else if first.contains("ja") {
                    "ja"
                } else {
                    "en"
                }
                .into()
            })
            .as_str()
    } else {
        settings.language.as_str()
    };
    if language == "zh-Hans" {
        return key.into();
    }
    MESSAGES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../frontend/messages.json"))
                .expect("bundled translations")
        })
        .get(key)
        .and_then(|v| v.get(language))
        .and_then(|v| v.as_str())
        .unwrap_or(key)
        .into()
}
