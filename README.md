# macntfs

免费开源的 macOS NTFS 桌面工具，Rust + Tauri 2 + Vue 3。支持 Apple Silicon，界面使用本机 WebKit，不依赖远程网页。

下载：[GitHub Releases](https://github.com/luckySpro/macntfs/releases/latest)。DMG 内的 PKG 包含应用、macFUSE、NTFS-3G 和 root 权限助手，终端用户安装和磁盘读写无需联网、Homebrew 或命令行。首次仍须完成 macOS 驱动授权，稳定内核模式可能需要重启。

目前为未 Apple Developer ID 签名公证的开发测试版本。FSKit 仅提供实验性选择，默认使用内核后端。尚未完成所有 macOS 版本兼容测试。

## 开发

需要 macOS Apple Silicon、Rust、Node.js、Xcode Command Line Tools。开发机器安装 autoconf、automake、libtool、pkgconf；这些不会成为终端用户依赖。

```sh
npm ci
python3 scripts/prepare-vendor.py
# 安装 vendor/downloads/Install macFUSE.pkg 以提供构建 SDK
bash scripts/build-driver.sh
npm run build
cargo test --workspace
cargo build --release --workspace
python3 scripts/package.py
python3 scripts/validate-package.py
```

`npm run tauri -- dev` 启动开发界面。打包包内携带驱动和对应许可证、源码归档。`scripts/package.sh` 使用已有缓存进行离线构建。

## 版本与更新

`python3 scripts/version.py 0.3.1` 同步 VERSION、Rust 和前端版本，再运行 `npm install --package-lock-only` 和 `cargo check --workspace` 更新锁文件。提交后推送 `v0.3.1` 标签，GitHub Actions 会构建 PKG、DMG、最终 `.app.tar.gz` 签名和 `latest.json` 并创建 Release。

仓库 secret `TAURI_SIGNING_PRIVATE_KEY` 保存 Tauri 更新私钥；只提交公钥。私钥应额外安全备份，不能加入源码或 Release。签名覆盖最终应用及所有离线资源。客户端启动检查 GitHub，用户点击下载安装后校验签名并重启；网络失败不影响本地功能。root Runtime 通过内置系统安装器更新；版本不匹配时禁止启动旧助手。

本地发布：设置 `NTFS_UPDATE_KEY` 为私钥文件路径，运行 `python3 scripts/release.py`。Apple 正式签名另需 `NTFS_APPLICATION_SIGN_IDENTITY` / `NTFS_INSTALLER_SIGN_IDENTITY`，签名后仍须 Apple 公证并 staple，当前流程不宣称已公证。

## 磁盘安全与诊断

仅操作用户选择的外置物理 NTFS 分区。UUID 再验证、root 路径与权限校验、驱动 SHA256 校验，拒绝强制卸载、格式化、休眠绕过。探测返回码单独分类；真实休眠或损坏须由 Windows 正常关机/检查磁盘处理，软件不会自动修复。安全推出作用于整块物理磁盘的所有分区。

应用升级后组件需要管理员安装授权；macOS 驱动安全政策无法由应用绕过。

## 许可

应用与助手 MIT，NTFS-3G GPL/LGPL，macFUSE 保留其原始许可和签名。此项目免费非商业分发。macFUSE 商业捆绑需另行取得书面许可。上游资源版本和 SHA256 位于 `vendor/manifest.json`。

## macOS 拒绝访问磁盘设备 / Operation not permitted

驱动若在打开 `/dev/disk…` 时得到 EPERM，还未读取 NTFS，不代表 Windows 休眠。0.3.1 在驱动和助手两层修复该误判，保留真实休眠/缓存检查。

在设置页选择「打开完整磁盘访问」及「打开助手位置」，手动将 `/Library/Application Support/NTFS Desktop/Runtime/bin/ntfs-helper` 添加并开启。macOS 隐私授权与 root 管理员密码不同。当前临时签名的助手更新后，旧授权可能因 code requirement 不匹配而失效；移除旧条目后重新添加，随后重启应用。完整磁盘访问权限较广，只为受信任的助手授权。正式发行需稳定的 Developer ID 签名。

macOS 专用驱动修改记录于 `scripts/patches/macos-device-permission.patch`。`scripts/test-probe-permission.py` 在临时文件上注入 EPERM，验证返回访问失败 19 而不是休眠 14，不操作真实磁盘。

应用显示名称、安装包及下载文件统一为 `macntfs`。为兼容已安装版本，Bundle ID 和权限助手的历史安装路径保持一致。


## 菜单栏与自动读写（0.3.5）

从完整 PKG 安装后，从「应用程序」打开 macntfs。菜单栏实时显示 NTFS 磁盘，默认在插入后自动检查并开启读写；可在菜单栏或设置关闭自动读写。关闭主窗口会保留菜单栏，退出应用会停止检测。后台助手不会强行结束已经挂载的驱动。

首次安装需系统管理员授权，以及 macOS 要求的磁盘访问许可。后续挂载通过系统管理的 root 助手执行，无需每次输入密码。安装窗口是否提供 Touch ID 由 macOS 决定，不修改 sudo、认证数据库或保存管理员密码。暂未设置自动登录启动，需要登录后打开一次应用。

安全边界：launchd root 助手仅提供状态检查和 NTFS 安全挂载，限制当前控制台用户、固定 /Applications/macntfs.app 路径、root 所有权和不可写应用文件、进程审计令牌与 Hardened Runtime 签名。开发目录中的 GUI、任意脚本和其他应用不能调用后台挂载。自动挂载失败会保留诊断，仅在重新插入或明确操作时重试。

0.3.5 起，应用自更新后需安装对应版本的完整 PKG，同步助手版本并恢复 root 所有权；仅安装内置 Runtime 不足以恢复自更新后的应用权限。日常磁盘挂载免密码，软件升级仍需系统安装授权。
