use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("src-tauri/tauri.conf.json")).unwrap())
            .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("dist/latest.json")).unwrap()).unwrap();
    let decode = |s: &str| String::from_utf8(STANDARD.decode(s).unwrap()).unwrap();
    let key = PublicKey::decode(&decode(
        config["plugins"]["updater"]["pubkey"].as_str().unwrap(),
    ))
    .unwrap();
    let signature = Signature::decode(&decode(
        manifest["platforms"]["darwin-aarch64"]["signature"]
            .as_str()
            .unwrap(),
    ))
    .unwrap();
    let version = manifest["version"].as_str().unwrap();
    let mut data =
        std::fs::read(root.join(format!("dist/NTFS-Desktop-{version}-arm64.app.tar.gz"))).unwrap();
    key.verify(&data, &signature, true).unwrap();
    assert!(signature.trusted_comment().contains(version));
    data[0] ^= 1;
    assert!(key.verify(&data, &signature, true).is_err());
    println!("PASS: published signature, signed version, modified archive rejection");
}
