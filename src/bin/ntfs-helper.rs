fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args == ["serve"] {
        macntfs_core::daemon::serve().map(|_| "服务已结束".into())
    } else {
        macntfs_core::privileged::execute(args)
    };
    let (ok, message) = match result {
        Ok(s) => (true, s),
        Err(s) => (false, s),
    };
    println!(
        "{}",
        serde_json::json!({"ok":ok,"message":message,"protocol":1})
    );
}
