NTFS Desktop v0.3.1（Apple Silicon 开发测试版）。

修复 macOS 权限被拒绝误报「Windows 休眠」：驱动区分打开设备时的 EPERM 与读取 NTFS 后发现的休眠状态，兼容旧驱动日志，并保留真实休眠/损坏保护。
增加「完整磁盘访问」设置入口和「打开助手位置」，不再显示 Some(14) 等 Rust 调试格式。

从 v0.3.0 更新后请安装内置组件；在系统设置 → 隐私与安全性 → 完整磁盘访问中，移除旧 ntfs-helper 条目并重新添加当前助手：
/Library/Application Support/NTFS Desktop/Runtime/bin/ntfs-helper

当前临时签名可能导致更新后的旧授权失效。完整磁盘访问须用户手动授权，软件不会自动修改隐私数据库、关闭 SIP 或绕过真实 NTFS 休眠检查。
尚未做 Apple Developer ID 签名公证。
