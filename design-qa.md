# macntfs 0.3.8 design QA

final result: passed

## Evidence and state

Source visual truth: the three approved image-generation results in `/Users/lucky/.codex/generated_images/01a11703-a6f9-7d32-a8ef-204c2d4fa5c6/`: `exec-a8391264-2788-4639-b2f9-5a87fb651df6.png` (Stone), `exec-8e38b904-0be5-4ace-996b-6f14d877c21e.png` (Office), `exec-f093e0cc-36cf-4485-88fc-178159bb0b19.png` (Graphite).

Implementation screenshots: `dist/design-qa/Stone.png`, `Office.png`, `Graphite.png`, corresponding `*-native.png`, and `settings.png`. Full-view comparison evidence: `compare-Stone.png`, `compare-Office.png`, `compare-Graphite.png` in the same directory, each contains the source and implementation together. All images were opened and visually inspected.

CSS viewport: 1440×1024, DPR 1; additional checks at 1060×760 (native default size) and 760×600 (minimum window). Source mockup pixels are 1488×1058; comparison boards scale both images proportionally, excluding interpretation of native titlebar chrome. Implementation full-page captures include natural vertical scroll content (Stone 1440×1101; Office/Graphite 1440×1024). Persistent sidebar/navigation and primary disk controls remain accessible; no horizontal overflow at any tested width.

State: one mounted writable BackUp NTFS volume, 511.7 GB, connected helper. Browser tests mock IPC; they do not establish real disk operation or native menu success. Settings, empty volumes, read-only and unmounted volumes, and switching the selected device were also checked.

## Findings and iteration

Initial P2: the first implementation was too dense at wide desktop sizes, Office lacked the intended selectable device list/detail composition, and the Graphite actions were not in the inspector. Fixed with wide-viewport typography/spacing, a real selectable list, selected-volume detail, and dedicated Graphite inspector actions. Added the generated silver disk asset and semantic native-style switches. Recaptured and inspected all three revised views, together with the source images.

Post-fix comparison: no remaining actionable P0/P1/P2 findings. The working application deliberately retains live component warnings, detailed device identity, refresh/theme controls, and disk protection information that the healthy-state mockups simplify. Office uses a denser list and slightly smaller illustration to support multiple devices. The native titlebar comes from Tauri rather than HTML.

## Required fidelity surfaces

- Typography: native macOS/PingFang stack, appropriate heading hierarchy, readable Chinese and metadata; responsive sizes preserve space at the default native window.
- Spacing/layout: stone sidebar/detail surface, office top navigation/list/detail, graphite device rail/detail/inspector are distinct and follow the approved compositions. Narrow widths reflow the inspector instead of clipping controls.
- Colors/tokens: warm stone, steel blue and graphite semantic palettes replace purple throughout, including the authorization guide. Graphite primary text was darkened for contrast.
- Images/icons: a generated transparent silver-drive asset is present in the live disk view; existing library icons remain crisp. No CSS approximation of the drive artwork.
- Copy/content: real volume names, capacity, identifier, mount path, read/write state and author information are preserved. The safety semantics of whole-disk eject remain explicit.

Focused inspection: full-resolution disk/operation views and the full settings capture were opened after the combined comparisons; text and control states were readable. No additional focused crops were necessary.

## Verification and limits

`node scripts/test-ui.cjs` passed with the bundled Playwright module: theme persistence across reload, Finder/eject action parameters, settings navigation, empty state, read-only/unmounted selection and mount action parameters, three viewport widths, and no page errors. The isolated test browser does not access physical disks. Native tray creation compiles; physical tray clicks, actual eject/remount, clean-machine installation and OS security changes were not exercised in this iteration.

Follow-up P3: native macOS icon material and illustration proportions can be refined after use. No blocking visual issues remain.

## 0.3.9 快捷面板增量复核

面板布局沿用已确认的配色与银色磁盘资产，参考用户提供的旧原生菜单，将平铺文本改为设备分组和图标按钮。隔离浏览器截图 `/tmp/macntfs-qa/panel.png`（420×540，DPR 1）及 CUA 原生截图（840×1080，Retina DPR 2）均已打开检查。三项操作在同一组内对齐，状态标签可识别，底部控制固定，多设备只在列表内滚动，无横向或整体纵向溢出。图标资源保持透明，中文文字清晰，深色主按钮使用深色文字保证对比。

测试实例实际 Esc 收起和恢复主窗口通过；多设备、忙碌与错误状态以模拟 IPC 验证。主应用图标采用同一用户指定图像生成 ICNS。无新增阻断视觉问题；final result: passed。

## 0.3.10 圆角与风格入口复核

用户截图中的圆角露白源于不透明的窗口与文档背景。快捷面板开启 Tauri macOS 透明窗口功能，并仅将面板 html/body/#app 背景设为透明；主窗口背景保持原主题。顶部原生下拉框移除，主题切换集中在设置页的卡片。三种风格切换与重载保存通过，1060×760 与 760×600 无横向溢出。`/tmp/macntfs-qa/panel-{Stone,Office,Graphite}.png` 以透明截图保存，Pillow 检查四角 alpha 均为 0。

CUA 查看实际 WebKit 主窗口和快捷面板，顶部按钮排布正常；原生截图为 RGB，捕获时透明区域显示白色，不能用此截图断言桌面合成后的角落颜色。原生窗口透明配置及 macos-private-api 编译通过。磁盘操作使用模拟 IPC 回归，无真实推出、挂载或写入。

## 0.3.11 原生标题栏配色

主窗口采用 Transparent 原生标题栏样式（非无边框、非覆盖内容），保留系统拖动和窗口控制。Rust 启动与保存主题时同步原生明暗外观及背景颜色。CUA 实际切换 Stone、Office、Graphite 并截图，检查标题栏采样点 (1800,30)，RGB 分别为 (250,250,248)、(248,249,251)、(32,36,39)，与主题画布颜色误差不超过 2。重启测试应用后深色标题栏恢复，RGB 同为 (32,36,39)。截图保存在 `/tmp/macntfs-qa/title-*.png`。测试结束恢复原 Graphite 选择；未操作真实磁盘。

## 0.3.12 多语言与必需更新

跟随系统、简体中文、繁体中文、英文、日文。翻译目录内置，保存选择，主窗口和菜单栏同步。隔离 UI 覆盖四种语言切换、重载、系统语言解析、三种主题、420×540 面板布局与必需更新的断网/重试/安全推出场景。技术详情保留原文。

原生 CUA 使用独立调试数据目录，确认日文缓存更新要求启动恢复、布局正常、Esc 不关闭，背景控件 inert。临时应用路径包含 /tmp 符号链接，Tauri 拒绝其更新启动路径，因此本实例未验证在线安装；没有真实磁盘或系统授权操作。
