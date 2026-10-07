macntfs v0.3.3（Apple Silicon 开发测试版）。

修复 Finder 只显示目录、不显示下级普通文件的问题：启用 macFUSE 扩展属性兼容处理。
桌面与 Finder 使用原 NTFS 磁盘名称和 macOS 外置磁盘图标，替代默认 macFUSE 卷外观。
已在临时 NTFS 镜像验证 Finder 显示中文子目录文件，以及读写、正常卸载和重新探测。

请安装新版离线 PKG 并完成组件更新，然后安全推出、重新连接磁盘并开启读写。已挂载的磁盘需要重新挂载才能应用修复。
当前未做 Apple Developer ID 签名公证。权限助手如失去完整磁盘访问授权，请按应用提示重新添加。
