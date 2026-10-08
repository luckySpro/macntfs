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


## 安装提示定位与复核

实机发现开发目录 0.3.6 测试 GUI 与 /Applications 中的 0.3.5 GUI 同时运行，后台助手为 0.3.6。关闭测试实例及旧正式进程，安装与线上包相同的 Application.pkg 后，正式应用和助手均为 0.3.6。从 /Applications 启动后，设置页实际显示「后台助手已就绪 · 日常挂载无需密码」，安装提示消失。这验证了正式 GUI 的审计身份、签名及后台状态连接；未据此宣称新挂载写入已经验证。

后续源码将 service_issue 返回界面，并明确报告 GUI/助手版本，不再将所有失败归为需要完整安装。12 项 Rust 测试、前端构建和 Clippy 通过。此诊断文案改进尚未发布到新的版本包；现有线上 0.3.6 安装正确后连接已通过实机复核。


## 0.3.7 更新与授权向导

原更新流程仅替换 GUI，不能同步助手。0.3.7 改用独立 latest-installer.json，通过 Tauri Update.download 下载并校验最终完整 PKG 的签名及版本，再打开系统安装器并退出旧 GUI；保留历史 latest.json 给旧客户端，迁移到 0.3.7 需完整 PKG。移除前端通用 updater IPC 权限，仅保留固定来源的受限更新命令。

本机 12 项 Rust 测试、Clippy（-D warnings）、前端构建和离线 release 构建通过。完整 PKG、旧应用归档签名验证均通过，篡改一个字节均被拒绝；安装包结构/权限/驱动签名检查通过。

原生 GUI 预览实际检查四步向导入口、恢复模式步骤展开和磁盘访问步骤，并截图确认布局。Apple 芯片恢复步骤依据 macFUSE 与 Apple 官方文档。向导不宣称能自动判断完整的内核批准或 TCC 状态，不执行系统安全更改。未实际进入恢复模式、重启系统或变更安全策略。

真实 GitHub 更新链路测试由 examples/verify-live-updates.rs 进行：模拟 0.3.6 应用版本，检查更新、下载发布的完整 PKG，调用实际 Tauri 签名验证，并比较下载字节与本地发布文件。不安装或改动正式应用；测试结果在发布后另记。尚未执行从旧应用经新前端调用安装器并由用户完成授权的完整升级。


0.3.7 发布后线上复核：九个 GitHub Release 资源的服务端 SHA256 均与本地文件一致。实际 Tauri updater 以 0.3.6 的 package version 检查 GitHub latest-installer.json，发现 0.3.7，并成功下载完整 PKG，通过签名与版本验证，下载字节与本地发布 PKG 完全一致。

本机通过系统安装器同步应用组件后，正式 /Applications GUI 和助手均为 0.3.7。正式 GUI 设置页实际显示「当前应用 v0.3.7 · 已安装助手 v0.3.7 · 已连接 · 已是最新版本」，验证新版正常的同版本更新检查与助手连接。没有为验证向导而进入恢复模式或更改安全策略；完整新前端下载按钮到安装器的自动交接仍未作为跨版本实机安装测试执行。

## 0.3.8 三套主题与菜单栏操作

新增暖灰原生、简洁办公、深色工作台；配色与布局一起切换，主题写入已有本机设置文件，旧设置兼容。菜单栏取消磁盘子菜单，直接显示各磁盘的 Finder 打开、开启读写和安全推出入口；主窗口、菜单栏、自动挂载共用互斥操作状态，切换自动读写开关不再清除正在执行的操作状态。

13 项 Rust 单元测试、Clippy（workspace/all-targets，-D warnings）、Vue 生产构建与离线 release 构建通过。隔离浏览器模拟 IPC，验证三套主题保存与重载、多设备选择、读写/未挂载/空状态、Finder 与推出的参数、设置页以及 1440、1060、760 宽度无横向溢出。已检查截图并与已确认效果图比较，详见 design-qa.md。

完整离线 PKG 结构、root 权限、GUI Hardened Runtime、内置组件哈希及原厂 macFUSE 签名校验通过。本轮没有对用户实体磁盘执行推出、写入、挂载，也没有安装到 /Applications、重启或改变系统授权。菜单栏真实点击与干净系统安装尚未实机验证；不能把模拟 IPC 测试当作这些操作已经成功。

0.3.8 发布后复核：九个 GitHub 资源的服务端 SHA256 均与本地最终文件一致。真实 Tauri updater 模拟 0.3.7 客户端，通过 latest-installer.json 检测到 0.3.8，下载完整 PKG，并通过版本绑定签名验证，字节与本地发布包完全一致。刚发布时 latest 地址曾短暂返回旧清单，发布传播完成后再次检查通过。没有执行安装。

## 0.3.9 主图标与分组快捷面板

应用 ICNS 从用户选定的银色磁盘透明 PNG 生成，覆盖完整 macOS 图标尺寸。新增独立 Tauri 快捷窗口，左键菜单栏触发，右键保留原生菜单。使用实际菜单栏图标位置与显示器工作区定位，支持 Retina 坐标换算与屏幕边缘限制；多显示器混合缩放尚未实机测试。

隔离浏览器模拟 IPC：主界面三种主题、面板 Finder/推出/挂载操作参数、自动读写开关保存、主窗口/设置/Esc 命令、多设备滚动、操作期间禁用、失败反馈及解锁均通过。没有在用户磁盘执行写入、推出或挂载。

原生测试实例使用调试入口 MACNTFS_PREVIEW_PANEL 显示与菜单栏同一窗口：实际渲染了 BackUp 磁盘、按钮组、状态及禁用的开启读写按钮；Esc 收起后没有可见窗口，点击「打开主窗口」恢复主界面。调试实例不符合后台助手受信任路径，连接提示为预期的未连接；没有据此宣称新版安装后的助手已连通。系统菜单栏左键点击、外部焦点收起和混合缩放多屏还未完成自动实机点击验证。

0.3.9 发布后复核：先将九个资源上传到 GitHub 草稿，逐一核对服务端 SHA256 与最终本地文件一致，再发布为 latest。真实 Tauri updater 模拟 0.3.8 客户端，检测到 0.3.9，下载完整 PKG，通过版本绑定签名验证，字节与本地发布包完全一致；没有执行安装。主窗口和面板采用同一银色磁盘标识。测试实例已退出并移入废纸篓，正式应用未替换。
