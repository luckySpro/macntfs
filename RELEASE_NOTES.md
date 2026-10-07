macntfs v0.3.4（Apple Silicon 开发测试版）。

修正 NTFS-3G 到 macFUSE 图标模块的参数传递，使用系统外置磁盘图标和原 NTFS 卷名。
采用 NTFS-3G 原生 macOS 扩展属性模式 streams_interface=openxattr，兼容 Finder 文件显示和卷图标标记。
回归测试同时检查中文下级文件、图标字节、Finder 图标标记及扩展属性读写。
修正驱动构建脚本重复运行时可能反向撤销已有补丁的问题。

安装后完成组件更新，安全推出并重新连接磁盘后开启读写；旧挂载会话不会自动替换。
当前没有 Apple Developer ID 签名公证。若助手失去完整磁盘访问授权，请按应用提示重新添加。
