# 0.2.0 验证记录

测试环境：Apple Silicon、macOS 27.0.1，现有 macFUSE 5.4.0。用户原有 BackUp 磁盘只作状态识别，未用于写入或重挂载测试。

| 项目 | 结果 |
| --- | --- |
| Rust 单元测试 | 6 项通过：磁盘标识、参数转义、内置/非 NTFS 拒绝、后端版本选择、权限助手操作白名单、组件清单路径拒绝 |
| Clippy / rustfmt | 通过，Clippy 开启 `-D warnings` |
| 只读诊断 | 实际识别外置 NTFS、安装状态与嵌入资源哈希 |
| 离线引擎 | 2026.9.28 源码构建；otool 确认没有 Homebrew/开发机动态库依赖 |
| 32 MB 临时镜像 | NTFS 探测、内核挂载、写入、读回、正常卸载、卸载后探测通过 |
| 无效 NTFS 镜像 | 引导扇区破坏后拒绝读写挂载 |
| 包内容 | macFUSE 两组件、root 运行组件、GUI 四项全部在 PKG 内，安装选择均指向本地文件 |
| 原厂签名 | 完整 macFUSE PKG 和 libfuse 原厂 Team ID 3T5GSNBU6W，原始签名校验通过 |
| 组件权限 | PKG BOM 中权限助手及引擎为 UID 0、0755；哈希清单 UID 0、0644 |
| 原厂组件完整性 | 一体 PKG 中 macFUSE Payload / Scripts / BOM 与原厂展开组件一致 |
| GUI | 实机截图检查中文无衬线字体、导航、磁盘卡片及状态；设置页/深色主题未作截图复核（随后系统锁屏） |
| 开发包签名 | 本机 ad-hoc 签名校验通过；不等于 Developer ID 公证 |

## 尚未通过或未执行

- FSKit：临时镜像实验未取得实际挂载，不作为已验证能力；当前默认稳定模式使用内核后端。
- 干净机器实际断网安装、首次系统扩展授权及恢复模式/重启流程：尚未执行。
- 主 PKG / GUI 的 Developer ID 签名、公证、Gatekeeper：本机无有效 Developer ID 证书，尚未完成。
- root 权限助手的完整安装后实盘集成：未对用户真实磁盘执行授权写入测试。
- Intel 与全部支持的 macOS：未验证。

资源准备阶段可联网下载并验证固定版本；最终打包命令使用离线 Cargo，运行安装计划不含远端下载项。此项属于构建与内容验证，不能代替干净机器的断网安装测试。

## 0.3.0 变更验证

迁移至 Tauri 2 + Vue 3，增加受限 IPC 命令、后台磁盘操作、自动刷新和可展开诊断。
更新机制：GitHub latest.json，Tauri 签名校验以及签名版本绑定；只发布最终包含离线资源的应用归档。
真实外置盘未被开发过程挂载、写入或修复；用户所报截断日志无法确定实际探测返回码。

本机 0.3.0：7 项 Rust 测试通过；clippy 无警告；前端生产构建通过；临时 NTFS 镜像读写/卸载/拒绝损坏检查通过。macOS 原生 Tauri 窗口已截图检查，实际扫描到 BackUp 只读磁盘，未对其执行写入。PKG 检查通过：离线安装计划、原始 macFUSE 内容未修改、签名、驱动哈希、root 权限和动态库依赖闭包。
最终更新归档验证通过：发布公钥验证成功、签名绑定 0.3.0、篡改一个字节后验证拒绝。DMG 内部校验通过。Apple Developer ID 和公证仍未具备；尚未对未来不同版本执行实际应用替换测试。

## 0.3.1 权限误判修复

用户提供 Error opening '/dev/disk4s3': Operation not permitted + 14。源码确认 ntfs_volume_error 将 EPERM 统一映射为 HIBERNATED；但该日志来自设备 open 失败、发生于 NTFS 元数据读取之前。
本机 tccd 日志确认 ntfs-helper 的 kTCCServiceSystemPolicyAllFiles code requirement 匹配失败，现有授权与更新后的临时签名不匹配。未修改 TCC 数据库或自动授予权限。
修复 macOS 驱动设备 open 的 EPERM → 访问失败映射；助手兼容旧输出，提供手动授权引导。临时文件 EPERM 注入回归测试通过，真实休眠检查保持不变。
0.3.1 本机验证：9 项 Rust 测试、clippy、临时文件 EPERM 注入、NTFS 镜像完整读写与卸载、离线 PKG 校验、更新签名及篡改拒绝均通过。真实磁盘未执行新的挂载/写入；修复后的实际挂载需用户重新授权助手后验证。


## 0.3.4 Finder 兼容验证

0.3.3 经正式 GUI / 已安装 root 助手重新挂载 BackUp 后，Finder 实际显示原先为空目录中的 5 个普通文件，桌面显示 BackUp 与系统外置磁盘图标。实盘仅用于挂载与读取元数据，未创建测试文件。

进一步检查发现上游 NTFS-3G 在 fuse_mount 后释放了库选项，导致 volicon 未送入 fuse_new；auto_xattr 又绕过了卷图标模块提供的根目录 FinderInfo。0.3.4 使用官方支持的 streams_interface=openxattr，并在 macOS 上保留图标库选项。仅保留 volicon，避免将 nonempty 等挂载选项错误送入 fuse_new。

临时 NTFS 镜像验证：Finder 显示两个中文/英文子目录文件，卷名 macntfs-Test 正确；虚拟 .VolumeIcon.icns 与系统 External.icns 完全一致；根目录 FinderInfo 为 32 字节且包含 0x0400 图标标记；普通文件缺失属性返回 ENOATTR（93）；原生扩展属性创建/读取/删除通过。此测试不在真实磁盘写入图标或测试属性。

来源：https://github.com/tuxera/ntfs-3g/wiki/Using-Extended-Attributes 与 https://github.com/macfuse/macfuse/wiki/Mount-Options 。

0.3.4 安装后实盘复测通过：正式 GUI 启动已安装的 root 助手，实际驱动参数为 streams_interface=openxattr、原卷名及系统 External.icns。Finder 实际显示原问题目录的 5 个普通文件；卷简介截图确认原名称与橙色系统外置磁盘图标。实盘虚拟图标字节与系统资源一致，根目录 FinderInfo 包含 0x0400 标记，已安装助手与驱动的哈希符合 0.3.4 清单。验证过程中未向实盘创建测试文件或写入图标文件。


## 0.3.5 菜单栏与后台助手

本机 Rust 12 项测试、Clippy（-D warnings）、前端生产构建、离线 release 构建通过。PKG 校验确认 launchd 配置、GUI Hardened Runtime、root 所有权、离线组件及既有驱动签名/哈希。

用户通过系统安装器安装后台组件后，实际 launchd 服务以 root 运行。普通 Python 进程发送状态/挂载请求均在身份检查阶段被拒绝，未执行磁盘操作。开发目录 GUI 可扫描真实 BackUp 的现有可读写挂载；关闭窗口后 GUI 进程继续运行。

正式应用尚未安装到 /Applications，本轮尚未完成受信任 GUI 的免密码挂载、物理拔插后自动挂载和菜单栏交互的完整实机验证。已将系统安装器交给用户完成安装；不把现有 0.3.4 的可写挂载算作新版自动挂载成功。Touch ID 是否出现由系统安装器决定，本轮未验证指纹授权。

应用自更新会改变应用文件所有权，必须安装对应完整 PKG，同步助手并恢复 root 权限；GUI 已显示引导，未宣称 root 助手可无授权自动升级。


## 0.3.6 Dock 生命周期修复

增加 Tauri Reopen 处理恢复主窗口；CloseRequested 继续隐藏窗口。实际测试发现 Cocoa terminate 绕过 Tauri ExitRequested，因此在现有应用 delegate 注册 applicationShouldTerminate，普通原生退出只隐藏窗口，显式菜单栏退出与更新重启保持 Tauri 程序退出。系统退出原因按 SDK 的 kAEQuitReason 放行关机、重启与注销；未执行实际关机或注销测试。

本机原生 GUI 验证：关闭窗口后后台仍在，macOS 再次打开恢复主窗口；Command-Q 后应用清单仍为运行中，进程 PID 58150 保持不变。恢复窗口后显示 0.3.6 和真实 NTFS 状态。Dock 右键菜单本身未直接点击；验证的是它使用的同一 Cocoa terminate 路径和 macOS reopen 路径。

12 项 Rust 测试、前端构建、Clippy（-D warnings）及离线 release 构建通过。本轮未卸载、写入或修复真实磁盘。
