//! Local, privacy-preserving health checks. Never probe raw disks, mount, or upload.
use crate::{
    settings::{Backend, Settings},
    system::{self, Environment, Result, Volume},
};
use serde::Serialize;
#[derive(Clone, Serialize)]
pub struct Check {
    pub code: &'static str,
    pub status: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub action: &'static str,
}
#[derive(Serialize)]
pub struct Report {
    pub schema: u8,
    pub version: &'static str,
    pub os: String,
    pub backend: String,
    pub checks: Vec<Check>,
    pub volumes: Vec<DiskSummary>,
}
#[derive(Serialize)]
pub struct DiskSummary {
    pub index: usize,
    pub mounted: bool,
    pub writable: bool,
}
fn check(
    code: &'static str,
    status: &'static str,
    title: &'static str,
    detail: &'static str,
    action: &'static str,
) -> Check {
    Check {
        code,
        status,
        title,
        detail,
        action,
    }
}
pub fn assess(
    env: &Environment,
    settings: &Settings,
    volumes: &[Volume],
    scan_ok: bool,
    verified_mounts: bool,
) -> Vec<Check> {
    let mut rows = vec![
        check(
            "runtime",
            if env.runtime { "pass" } else { "error" },
            "离线读写组件",
            if env.runtime {
                "组件完整性与版本检查通过"
            } else {
                "组件缺失、损坏或版本不一致，请安装完整 PKG"
            },
            if env.runtime { "" } else { "release" },
        ),
        check(
            "helper",
            if env.service { "pass" } else { "error" },
            "后台助手",
            if env.service {
                "已验证助手连接与版本"
            } else {
                "请退出旧实例，从「应用程序」启动；仍失败时安装完整 PKG"
            },
            if env.service { "" } else { "release" },
        ),
    ];
    if settings.backend == Backend::Microvm {
        rows.push(check(
            "backend",
            if crate::microvm::supported(&env.os) && env.microvm {
                "unknown"
            } else {
                "error"
            },
            "微虚拟机实验模式",
            if crate::microvm::supported(&env.os) && env.microvm {
                "离线镜像已提供；完整性与读写将在挂载时验证"
            } else {
                "需要 Apple Silicon、macOS 13+ 和完整离线镜像"
            },
            "",
        ));
    } else {
        rows.push(check(
            "driver",
            if env.fuse { "unknown" } else { "error" },
            "驱动授权",
            if env.fuse {
                "驱动已安装；macOS 授权状态需通过实际挂载确认"
            } else {
                "请安装内置 macFUSE，并按首次使用向导完成授权"
            },
            "settings",
        ));
    }
    rows.push(check(
        "permission",
        if verified_mounts { "pass" } else { "unknown" },
        "磁盘访问权限",
        if verified_mounts {
            "已观察到受管理磁盘的实际读写挂载"
        } else {
            "助手连接不能证明磁盘访问权限；连接磁盘并尝试挂载后确认"
        },
        "permissions",
    ));
    rows.push(check(
        "devices",
        if scan_ok { "pass" } else { "error" },
        "设备检测",
        if scan_ok {
            "已完成外置 NTFS 设备检测"
        } else {
            "设备信息读取失败，请刷新后重试"
        },
        "",
    ));
    rows.push(check(
        "mounts",
        if volumes.iter().any(|v| !v.mount.is_empty()) {
            "pass"
        } else {
            "unknown"
        },
        "挂载状态",
        if volumes.iter().any(|v| !v.mount.is_empty()) {
            "状态来自 macOS 实际挂载结果"
        } else {
            "尚未观察到已挂载磁盘"
        },
        "",
    ));
    rows
}
pub fn collect() -> Report {
    let env = system::environment();
    let settings = Settings::load();
    let scan = system::scan();
    let scan_ok = scan.is_ok();
    let volumes = scan.unwrap_or_default();
    let table = crate::sessions::mount_table().unwrap_or_default();
    let verified = volumes.iter().any(|v| {
        v.writable
            && crate::sessions::managed_mounts(&table)
                .iter()
                .any(|m| m.target == v.mount)
    });
    Report {
        schema: 1,
        version: env!("CARGO_PKG_VERSION"),
        os: env.os.clone(),
        backend: format!("{:?}", settings.backend),
        checks: assess(&env, &settings, &volumes, scan_ok, verified),
        volumes: volumes
            .iter()
            .enumerate()
            .map(|(i, v)| DiskSummary {
                index: i + 1,
                mounted: !v.mount.is_empty(),
                writable: v.writable && !v.mount.is_empty(),
            })
            .collect(),
    }
}
pub fn export() -> Result<String> {
    let dir = crate::settings::data_dir().join("Diagnostics");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let path = dir.join(format!("macntfs-diagnostic-{now}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&collect()).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    system::run("/usr/bin/open", &["-R", &path.to_string_lossy()])?;
    Ok("诊断报告已保存并在 Finder 中显示；未包含磁盘名称、UUID、文件路径或网络地址".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connected_helper_does_not_imply_disk_access_or_driver_authorization() {
        let env = Environment {
            runtime: true,
            service: true,
            fuse: true,
            ..Environment::default()
        };
        let rows = assess(&env, &Settings::default(), &[], true, false);
        assert_eq!(
            rows.iter().find(|c| c.code == "permission").unwrap().status,
            "unknown"
        );
        assert_eq!(
            rows.iter().find(|c| c.code == "driver").unwrap().status,
            "unknown"
        );
        let v = Volume {
            id: "secret".into(),
            uuid: "private".into(),
            name: "personal".into(),
            parent: String::new(),
            mount: "/private/path".into(),
            size: 0,
            free: 0,
            writable: true,
        };
        let report = Report {
            schema: 1,
            version: "test",
            os: "26".into(),
            backend: "Auto".into(),
            checks: rows,
            volumes: vec![DiskSummary {
                index: 1,
                mounted: !v.mount.is_empty(),
                writable: true,
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        for private in [&v.id, &v.uuid, &v.name, &v.mount] {
            assert!(!json.contains(private));
        }
    }
}
