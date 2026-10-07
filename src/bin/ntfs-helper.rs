fn main() {
    let result = macntfs_core::privileged::execute(std::env::args().skip(1).collect());
    let (ok, message) = match result {
        Ok(s) => (true, s),
        Err(s) => (false, s),
    };
    println!(
        "{}",
        serde_json::json!({"ok":ok,"message":message,"protocol":1})
    );
}
