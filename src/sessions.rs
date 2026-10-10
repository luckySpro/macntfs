//! Reconcile our root-owned session records with the actual macOS mount table.
use crate::system::{RUNTIME, Result, Volume};
use serde::{Deserialize, Serialize};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub uuid: String,
    pub backend: String,
    pub target: String,
}
#[derive(Clone, Debug)]
pub struct MountEntry {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub writable: bool,
}
pub fn parse_mounts(text: &str) -> Vec<MountEntry> {
    text.lines()
        .filter_map(|line| {
            let (source, rest) = line.split_once(" on ")?;
            let (target, flags) = rest.rsplit_once(" (")?;
            let flags: Vec<_> = flags.strip_suffix(')')?.split(", ").collect();
            Some(MountEntry {
                source: source.into(),
                target: target.replace("\\040", " ").replace("\\011", "\t"),
                kind: flags.first()?.to_string(),
                writable: !flags.contains(&"read-only"),
            })
        })
        .collect()
}
pub fn mount_table() -> Result<Vec<MountEntry>> {
    crate::system::run("/sbin/mount", &[]).map(|s| parse_mounts(&s))
}
fn path() -> std::path::PathBuf {
    Path::new(RUNTIME).join("sessions.json")
}
pub fn load() -> Result<Vec<Session>> {
    if !path().exists() {
        return Ok(Vec::new());
    }
    crate::privileged::check_root_path(&path())?;
    let bytes = fs::read(path()).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("挂载记录格式无效".into());
    }
    let sessions: Vec<Session> = serde_json::from_slice(&bytes).map_err(|_| "挂载记录格式无效")?;
    if sessions.iter().any(|s| {
        !crate::system::valid_id(&s.id)
            || !["kernel", "fskit", "microvm"].contains(&s.backend.as_str())
            || s.target != target(&s.id, &s.backend)
    }) {
        return Err("挂载记录格式无效".into());
    }
    Ok(sessions)
}
pub fn target(id: &str, backend: &str) -> String {
    format!(
        "/Volumes/{}-{id}",
        if backend == "microvm" {
            "macntfs-VM"
        } else {
            "NTFS"
        }
    )
}
pub fn validate_mounted_backend(
    volume: &Volume,
    requested: &str,
    sessions: &[Session],
) -> Result<()> {
    let recorded = sessions
        .iter()
        .find(|s| s.id == volume.id && s.uuid == volume.uuid && s.target == volume.mount);
    let actual = recorded
        .map(|s| s.backend.as_str())
        .or_else(|| (volume.mount == target(&volume.id, "kernel")).then_some("kernel"));
    if actual.is_some_and(|backend| backend != requested) {
        return Err("请先安全推出磁盘，再切换读写模式".into());
    }
    Ok(())
}
pub fn record(volume: &Volume, backend: &str) -> Result<()> {
    let mut sessions = load()?;
    sessions.retain(|s| s.id != volume.id);
    sessions.push(Session {
        id: volume.id.clone(),
        uuid: volume.uuid.clone(),
        backend: backend.into(),
        target: target(&volume.id, backend),
    });
    let temporary = Path::new(RUNTIME).join("sessions.tmp");
    if temporary.exists() {
        crate::privileged::check_root_path(&temporary)?;
    }
    fs::write(
        &temporary,
        serde_json::to_vec(&sessions).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, path()).map_err(|e| e.to_string())
}
pub fn reconcile(volumes: &mut [Volume], sessions: &[Session], table: &[MountEntry]) {
    for v in volumes {
        if let Some(s) = sessions
            .iter()
            .find(|s| s.id == v.id && s.uuid == v.uuid && s.backend == "microvm")
            && let Some(m) = table
                .iter()
                .find(|m| m.target == s.target && m.kind == "nfs" && is_loopback_source(&m.source))
        {
            v.mount = s.target.clone();
            v.writable = m.writable;
        }
    }
}
pub fn is_loopback_source(source: &str) -> bool {
    source
        .split_once(':')
        .and_then(|(ip, _)| ip.parse::<std::net::Ipv4Addr>().ok())
        .is_some_and(|ip| ip.is_loopback())
}
pub fn managed_mounts(table: &[MountEntry]) -> Vec<&MountEntry> {
    table
        .iter()
        .filter(|m| {
            ["/Volumes/NTFS-", "/Volumes/macntfs-VM-"]
                .iter()
                .any(|prefix| {
                    m.target
                        .strip_prefix(prefix)
                        .is_some_and(crate::system::valid_id)
                })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconcile_requires_disk_identity_exact_target_and_private_nfs() {
        let mut v = Volume {
            id: "disk4s3".into(),
            uuid: "a".into(),
            parent: "disk4".into(),
            name: "BackUp".into(),
            mount: String::new(),
            size: 0,
            free: 0,
            writable: false,
        };
        let s = Session {
            id: v.id.clone(),
            uuid: v.uuid.clone(),
            backend: "microvm".into(),
            target: target(&v.id, "microvm"),
        };
        for source in ["127.0.0.2:/mnt", "192.168.1.2:/mnt"] {
            v.mount.clear();
            v.writable = false;
            let table = parse_mounts(&format!("{source} on {} (nfs, nodev, nosuid)", s.target));
            reconcile(
                std::slice::from_mut(&mut v),
                std::slice::from_ref(&s),
                &table,
            );
            assert_eq!(v.writable, source.starts_with("127."));
        }
        v.mount.clear();
        v.writable = false;
        v.uuid = "changed".into();
        reconcile(
            std::slice::from_mut(&mut v),
            &[s],
            &parse_mounts("127.0.0.2:/mnt on /Volumes/macntfs-VM-disk4s3 (nfs)"),
        );
        assert!(v.mount.is_empty());
    }
    #[test]
    fn update_guard_includes_legacy_mounts_but_ignores_other_shares() {
        let mounts = parse_mounts(
            "/dev/disk4s3 on /Volumes/NTFS-disk4s3 (macfuse, local)\n127.0.0.2:/mnt on /Volumes/macntfs-VM-disk5s1 (nfs, read-only)\nnas:/data on /Volumes/Work (nfs)",
        );
        assert_eq!(managed_mounts(&mounts).len(), 2);
        assert!(!mounts[1].writable);
    }
    #[test]
    fn refuses_cross_backend_reuse_including_legacy_mounts() {
        let mut volume = Volume {
            id: "disk4s3".into(),
            uuid: "same".into(),
            parent: "disk4".into(),
            name: "Test".into(),
            mount: target("disk4s3", "kernel"),
            size: 0,
            free: 0,
            writable: true,
        };
        assert!(validate_mounted_backend(&volume, "kernel", &[]).is_ok());
        assert!(validate_mounted_backend(&volume, "microvm", &[]).is_err());
        volume.mount = target(&volume.id, "microvm");
        let record = Session {
            id: volume.id.clone(),
            uuid: volume.uuid.clone(),
            backend: "microvm".into(),
            target: volume.mount.clone(),
        };
        assert!(
            validate_mounted_backend(&volume, "microvm", std::slice::from_ref(&record)).is_ok()
        );
        assert!(validate_mounted_backend(&volume, "kernel", &[record]).is_err());
    }
}
