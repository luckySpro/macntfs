# macntfs【不好用就自己写系列之mac上的ntfs硬盘使用工具】

## 0.3.17 微虚拟机启动修复

0.3.16 在真实 launchd 助手中启动微虚拟机会在解析调用用户时失败：anylinuxfs 原本从 sudo 环境或父进程查找普通用户，而 launchd 助手的父进程均为 root。0.3.17 从系统 `/dev/console` 获取已登录用户 UID/GID，在清空继承环境后作为 anylinuxfs 所需的身份信息传入；不运行 sudo、不保存密码、不接受客户端传来的身份，也不修改系统认证策略。

失败时「技术详情」显示本次调用的退出状态和最多 8 KiB 日志，不混入历史尝试，不再笼统提示磁盘权限。24 项 Rust 测试与严格 Clippy 通过；新增真实子进程环境验证及日志隔离/限长测试。完整包更新后才能由正式助手应用修复；实际外置 NTFS/NFS 挂载仍待安装后复测，保留实验性标记。

## 0.3.16 双模式开发预览

同一个应用和版本管理两种后端，在「设置与更新 → 读写模式」选择：

- **稳定模式**：macFUSE + NTFS-3G，保留 0.3.15 的 1 MiB I/O 优化，支持插入后自动读写。首次驱动授权可能需要恢复模式设置及重启。
- **免内核扩展模式（实验性）**：内置 Linux 微虚拟机 + 本机 NFS，Apple Silicon、macOS 13+，无需 macFUSE 扩展或降低启动安全性。仍需管理员安装及助手磁盘访问授权，目前仅手动挂载。

完整 PKG 的「安装类型」页可勾选或取消「稳定模式组件 · macFUSE」。macOS 13+ 新机器默认不勾选；已有 macFUSE 的机器默认保留组件更新；更高版本不会降级。macOS 12 必须选择稳定组件。不勾选不会卸载已有 macFUSE；两种用户均获得同一应用、助手与离线微虚拟机组件。

已有设置在升级后保留。没有设置且未安装 macFUSE 的兼容机器，首次启动选择免内核模式；其他机器选择稳定模式。已有 macFUSE 但想使用免内核模式的用户需在应用明确选择，不会因取消安装组件而更改旧设置。免内核模式不提示安装或授权 macFUSE，首次使用向导改为确认模式及助手权限。

**切换前先安全推出所有受管理磁盘**，切换后重新连接并开启读写。挂载、启动或退出服务尚未完成时，后台也会拒绝切换；已挂载磁盘不能被当成另一后端的成功结果。失败后显示原因，不自动回退到另一模式。稳定模式会重新插入自动挂载；免内核模式需手动「开启读写」。取消稳定组件后，日后需要稳定模式可通过应用内「安装组件」打开本地 macFUSE 安装器，无需网络。

更新前先安全推出磁盘、从菜单栏退出，再安装完整 PKG；临时签名助手更新后，可能需要重新添加完整磁盘访问授权。免内核模式仍为开发预览，不保证与稳定模式具有相同速度或兼容性。本轮未更新线上必需更新清单。

验证：`cargo test --offline --workspace`、`node scripts/test-installer-modes.cjs`、`python3 scripts/validate-package.py`、`python3 scripts/test-microvm.py`。安装选择脚本及模拟界面通过不等于在没有 macFUSE 的全新机器完成真实 NTFS/NFS 挂载；物理磁盘端到端测试仍待安装新版后进行。
核心解决问题：
1、mac上读写NTFS分区
2、解决了原来使用时需要下载多个软件，用一个包把所有的进行集成；


作者：Lucky

邮箱：473276@qq.com

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

仓库 secret `TAURI_SIGNING_PRIVATE_KEY` 保存 Tauri 更新私钥；只提交公钥。私钥应额外安全备份，不能加入源码或 Release。签名覆盖最终应用及所有离线资源。客户端启动检查 GitHub；0.3.7 起下载完整 PKG 并验证签名及版本，再由系统安装器同步升级 GUI 与助手。安装后从「应用程序」启动。未发现必需更新时网络失败不影响本地功能；已知必需更新会保持挂载限制。版本不匹配时禁止启动旧助手。

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

从完整 PKG 安装后，从「应用程序」打开 macntfs。菜单栏实时显示 NTFS 磁盘，默认在插入后自动检查并开启读写；可在菜单栏或设置关闭自动读写。关闭主窗口、Dock 退出或 Command-Q 会保留菜单栏；菜单栏「退出 macntfs」会停止检测。后台助手不会强行结束已经挂载的驱动。

首次安装需系统管理员授权，以及 macOS 要求的磁盘访问许可。后续挂载通过系统管理的 root 助手执行，无需每次输入密码。安装窗口是否提供 Touch ID 由 macOS 决定，不修改 sudo、认证数据库或保存管理员密码。暂未设置自动登录启动，需要登录后打开一次应用。

安全边界：launchd root 助手仅提供状态检查和 NTFS 安全挂载，限制当前控制台用户、固定 /Applications/macntfs.app 路径、root 所有权和不可写应用文件、进程审计令牌与 Hardened Runtime 签名。开发目录中的 GUI、任意脚本和其他应用不能调用后台挂载。自动挂载失败会保留诊断，仅在重新插入或明确操作时重试。

0.3.5 至 0.3.6，应用自更新后需安装对应版本的完整 PKG，同步助手版本并恢复 root 所有权；仅安装内置 Runtime 不足以恢复自更新后的应用权限。日常磁盘挂载免密码，软件升级仍需系统安装授权。


## 完整安装包更新与首次向导（0.3.7）

从 0.3.6 及更早版本迁移，请直接安装 GitHub Release 的完整 PKG。0.3.7 的更新检查使用 `latest-installer.json`，通过 Tauri 下载并验证最终 PKG 与签名中的版本，打开系统安装器后退出旧 GUI。完成安装再从「应用程序」启动，应用和 root 助手由同一个完整 PKG 更新。旧 `latest.json` 应用归档保留供历史客户端使用，不代表旧客户端会自动升级助手。

「首次使用向导」包含安装、驱动授权、磁盘访问与完成检查。Apple 芯片稳定内核模式的恢复步骤按 [macFUSE 官方指南](https://github.com/macfuse/macfuse/wiki/Getting-Started) 和 [Apple 启动安全策略](https://support.apple.com/zh-cn/guide/mac-help/mchl768f7291/mac) 编写；详细说明降低安全性及允许用户管理内核扩展。Intel 不适用 Apple 芯片的步骤，FSKit 不要求恢复模式但本应用实现仍为实验性。应用不会代替用户修改系统安全设置。

### 界面与菜单栏

「设置与更新 → 界面风格」可切换暖灰原生、简洁办公、深色工作台，选择自动保存。办公风格通过列表选择磁盘，深色工作台通过设备侧栏选择磁盘。

点击 macntfs 菜单栏图标，各磁盘下直接提供「在 Finder 中打开」「开启读写」「安全推出整块磁盘」。安全推出会推出该磁盘的其他分区；操作期间暂停重复操作。关闭主窗口后这些功能继续可用。

前端回归检查：先运行 `npm run dev -- --port 1420`，再运行 `node scripts/test-ui.cjs`。需可用的 Playwright 与 Chrome；可通过 `PLAYWRIGHT_MODULE`、`CHROME_EXECUTABLE` 和 `TEST_UI_URL` 指定路径。测试模拟 IPC，验证主题保存、多磁盘选择、空状态、操作参数及窗口宽度，不会访问实体磁盘。

0.3.9 起，左键点击菜单栏图标会打开带图标和按钮的分组快捷面板；右键仍保留原生菜单。面板按磁盘显示状态，可打开 Finder、开启读写、安全推出，以及切换插入自动读写。Esc 或点击其他窗口会收起面板。

## 多语言与必需更新（0.3.12）

支持简体中文、繁体中文、英文和日文，默认跟随系统，在「设置与更新 → 语言」手动切换。语言保存在本机，主界面、快捷面板、原生右键菜单和授权向导共用内置翻译。系统或驱动返回的原始技术详情保留原文。

启动与后台每小时检查 GitHub 完整安装包清单。发现新版后，主界面显示不可跳过的更新提示，Rust 后端暂停新的手动和自动读写挂载；保留安全推出，已挂载磁盘不会被强行卸载。下载失败可重试，已知更新要求在断网或重启后仍保留，安装达到要求的版本后清除。没有检测到必需更新时仍可离线使用。

安装包经过版本签名验证，系统安装器仍由用户完成管理员认证。更新机制不替代 macOS 兼容性适配；未发布适配版本时，不能保证系统升级后驱动可用。0.3.11 及更早客户端的历史更新提示不会被远程改为强制；安装 0.3.12 后启用此策略。


## 0.3.13 开发预览：诊断、安全更新与离线微虚拟机

设置页提供本地诊断，分别显示组件、助手、驱动、磁盘访问、设备发现和挂载状态。助手已连接不再被解释为磁盘权限已取得；尚未实际验证的项目显示“未确认”。导出的 JSON 不包含卷名、UUID、设备路径、文件名或原始错误，也不会上传。

更新前必须安全推出本应用管理的卷；检测到驱动/VM 仍运行或正在挂载时停止替换组件。手动 PKG 安装也先运行独立更新保护组件，然后才安装 macFUSE。等待超时不会强行结束写入。

Apple Silicon / macOS 13+ 可手动选择 MicroVM 实验模式。Linux、ntfs-3g 和 NFS 组件预置在本地，不修改 SIP、恢复模式启动策略、PF 或 VPN 路由；仍需安装管理员授权和磁盘访问许可。通过明确的回环地址传输，网络卷仍可被其他本机进程访问。该模式关闭自动挂载，不会替换默认稳定模式。

已用临时 NTFS 镜像验证离线启动、下级文件读写、同步、卸载与重新探测。macOS NFS/真实设备热插拔尚未实测，Word 直接编辑及大批量文件传输仍有上游限制。每个分区的虚拟机内存上限为 512 MiB。

开发构建：安装 util-linux/gettext/llvm/lld/pkg-config，运行 `python3 scripts/build-microvm.py`，然后 `bash scripts/package.sh`。构建阶段需要网络，最终运行包不包含镜像初始化器。完整 GPL 对应源码归档尚待收集，发布流水线会阻止缺失源码的二进制发布。参见 [第三方说明](THIRD_PARTY_MICROVM.md)。

## 0.3.15 开发预览：大文件写入优化

稳定内核模式改用 1 MiB I/O 块并启用 NTFS-3G `big_writes`，减少大文件复制时的内核与用户态往返。FSKit 与微虚拟机参数保持原状，磁盘状态检查和 `norecover` 继续生效。完整 PKG 更新助手后，正常推出并重新连接磁盘才会应用新参数。

本机同盘实测：旧版单轮 1 GiB 写入约 69 MiB/s，新版三轮约 155–167 MiB/s，SHA-256 校验全部通过。128 MiB 短测没有改善；测试受缓存、文件分配位置和设备状态影响，不保证超大文件也能达到同样速度。详细方法、数字与限制见 [性能检查记录](PERFORMANCE.md)。暂不作为必需更新发布。

开发回归：`python3 scripts/test-offline-runtime.py --large-io` 检查嵌套文件、图标和扩展属性；`python3 scripts/test-write-performance.py` 在独立 8 GiB 稀疏镜像上进行三轮写入对比，正常卸载后检查卷状态。两个命令均不挂载实体磁盘。

### 0.3.18 MicroVM installation repair

Installer may omit Linux guest permission attributes, including the executable mode of `/bin/mount`. The package now carries integrity-checked guest metadata; the root postinstall helper restores only guest override attributes without adding macOS setuid permissions. Failed NFS confirmation includes the current attempt log. Physical MicroVM/NFS validation remains pending installation.

### 0.3.19 host image access correction

The installed 0.3.18 guest mount executable remained root-owned host mode 0600. Guest override attributes alone do not provide host file access. All regular guest image files are now host-readable (0644 plus existing execute bits), directories searchable (0755), with root ownership, no group/other writes and no host setuid/setgid. Guest permissions remain separately restored from the verified metadata. Package validation checks every unpacked image file for ordinary-user access. Physical host NFS validation is still pending installation.

### 0.3.20 immutable guest startup directories

Installed 0.3.19 advances past mount execution but vmproxy cannot create `/etc/lvm/archive` in the root-owned host image. Packaging now precreates all eight vmproxy temporary filesystem mount points with guest root metadata. The disposable package-derived VM test makes host image directories non-writable before startup, preventing caller-owned test fixtures from masking this issue. Physical host NFS mounting remains unverified until installation.
