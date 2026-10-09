fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: verify-microvm <payload-root>");
    macntfs_core::microvm::verify_manifest(std::path::Path::new(&path), false)
        .expect("offline VM payload verification failed");
    println!("PASS: complete offline VM manifest, files and symlink targets");
}
