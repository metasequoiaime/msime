# Agent Note: Windows 设置的页首引导与侧栏产品图标

Status: implemented

## Problem

macOS 设置在常用页面的正文顶部放一行页首引导：左边一句 13px 的说明，右边一张 80px 的水杉娘场景插画，窗口矮于 700px 时插画缩到 56px，「关于」页没有这一行（[幼年水杉娘设置元素](2026-10-19-settings-childhood-mascot.md)）。侧栏用的是产品自带的一套 SVG 图标。Windows 原生设置窗口在[行与组密度对齐](2026-10-20-windows-settings-mac-density.md)之后，正文顶部只剩一行 Windows 自己写的页面说明，文案和 macOS 不同（账号页「登录并管理跨设备同步与云端内容。」对 macOS 的「管理账号与跨设备设置同步。」），没有插画，「关于」页也有说明；侧栏图标是 Segoe Fluent Icons 字形。用户要求以 macOS 为准迁移。

## Decision

### 页首引导

- 文案与插画场景放在 `platforms/windows/settings/SettingsMascotIntro.h`，逐字取自 `packages/ui/src/settings/settings-mascot-intro.tsx` 的 `introductions` 和 `tabIntroductions`，页面 id 换成本窗口的。头文件不依赖 Windows，`tests/ui/settings_navigation.cpp` 直接核对。
- `mascot_intro(page, tab)` 按页面和页内标签取引导；`tab` 为空或这一页没有这个标签时取这一页的第一条，「关于」和不认识的页面没有引导。「语音输入」「键盘与手写」「剪贴板」在共享 UI 里按标签各有一句（输入 / 录音、键盘 / 手写、剪贴板 / 历史记录）。「键盘与手写」「剪贴板」接上[页内标签框架](2026-10-20-windows-settings-page-tabs.md)后显示当前标签那句；「语音输入」当时在 Windows 上还没有页内标签，显示第一个标签那句。后来语音页也分成「识别与输入」「录音行为」两个标签，现在同样显示当前标签那句，「服务配置」子视图（标签 id `service`）仍显示「识别与输入」那句，见 [Windows 设置的语音页标签与服务配置子视图](2026-10-20-windows-settings-voice-ai-mac-parity.md)。
- `main.cpp` 的 `make_page_header` 画这一行：说明 13 DIP、1.625 倍行高、次要色，插画 80 DIP，两者竖直居中、相隔 20，整行左右各留 4（macOS 的 `px-1`、`gap-5`）。`update_intro_size` 在窗口内容区矮于 700 DIP 时把插画缩到 56、间隔收成 12，`build_shell` 给根元素挂 SizeChanged 跟着改。插画对 UIA 是 Raw 视图，与共享 UI 的 `aria-hidden` 相同。
- `show_page_intro_tab(tab)` 只换说明文字和插画、不重画整页；`make_page_header` 用这一页在 `page_tabs_` 里记下的标签调用它，切标签时整页重画，引导随之换掉。
- 说明仍带 `settings-page-<页面 id>` 的自动化 ID；「关于」没有引导，这个 ID 落在品牌卡的产品名上，`settings_ui.ps1` 的切页等待因此不用分支。
- `PageLabel` 去掉 `summary` 字段：它唯一的用处就是原来那行说明。

### 插画资源

- `packages/ui/src/assets/mascot/*.webp` 的十个场景用 `dwebp` 转成 PNG，放在 `platforms/windows/settings/assets/mascot/`，由 `Settings.rc` 打成 RCDATA 110–119，资源号是 110 加上 `MascotScene` 的序号。十张一起打包（合计约 0.9 MB），不只打页首用到的六张：词库页的空状态等后续迁移要用 `empty`、`offline`、`updated`。
- `load_mascot` 用 `FindResourceW` 取资源字节，和品牌图标共用 `load_png` 解码；资源缺失或解不开时插画位置留空，说明照常显示。解出的位图按场景缓存在 `mascot_bitmaps_` 里：切标签、改设置后的刷新都会重画页首、新建 `Image`，每次重新解码的话插画要空一两帧才出来，看起来在闪；缓存命中时同步挂上，只有每个场景第一次出现时异步解码。macOS 的 `<img>` 由 WebView 的图片缓存兜住，没有这个问题。
- `Settings.rc` 的注释保持 ASCII：`rc.exe` 按构建机的 ANSI 代码页读这个文件。

### 侧栏图标

- 16 页的图标取共享 UI `app-resources.ts` 的 `settingsIcons`（按 `settings-page-registry.ts` 对应到页面），转成 PathIcon 的路径数据放在 `SettingsNavIcons.h`：描边用 picosvg 0.23 展开成填充，各图层用 skia-pathops 0.9 合并成一条非零环绕路径，坐标统一到 24 × 24 的方框，12 × 12 的图源放大一倍。转换后的轮廓与图源在 96px 下逐个渲染比对过 alpha 通道，差别只在抗锯齿边缘。
- `make_nav_icon` 用 `XamlReader::Load` 解析路径标记，给几何加 `size / 24` 的缩放，并把图标定成 15 × 15。导航项模板把图标装进 15 DIP 高的 Viewbox，定死尺寸以后 Viewbox 不按路径的外框把图标放大，图源四周的留白得以保留，和 macOS 的 15px `<img>` 一样大。前景交给模板（正文色），不透明度 0.9，与 macOS `navIcon` 的 `opacity-90` 相同。
- `PageLabel.glyph` 留着：交给共享应用的整页行和插件页的插件包行仍用 Segoe Fluent Icons 字形。

### 一致性检查

`scripts/test-windows-settings-art.py` 核对：`MascotScene` 与共享插画和 `core/mascot.tsx` 的场景一致；`Settings.rc` 的资源号按枚举顺序；打包的 PNG 与 WebP 同名同尺寸；每句引导连同场景都出自 `settings-mascot-intro.tsx`，没有标签的页面用的是经路由别名对应的共享页面那句；每个侧栏图标的图源文件存在，且是共享 UI 给对应页面的那张。引导条目的提取接受单行和多行排版及尾随逗号，文案与场景仍须逐字一致。主题和候选窗口的说明沿用共享页面合并重复说明后的具体文案。

## Alternatives considered

- **SVG 原样打进资源，用 `ImageIcon` 加 `SvgImageSource` 画**：图源与共享 UI 是同一份文件，不用转换，留白也天然保留。否决：`SvgImageSource` 不能换前景色，要跟随主题只能在运行时替换 SVG 里的颜色串，选中、悬停时也不走模板的前景键；三张图源只有 `viewBox` 没有宽高，`SvgImageSource` 的自然尺寸要另外指定，经 Viewbox 缩放后能不能按屏幕缩放重新栅格化，本机编不了 WinUI 无从验证。
- **PathIcon 不定尺寸，让 Viewbox 按路径外框缩放**：少写两行。否决：路径外框不含图源的留白，每个图标都会被放大到填满 15 DIP，粗细和视觉大小各不相同。
- **插画保留 WebP，直接交给 `BitmapImage`**：资源小得多（十张合计约 150 KB）。否决：WinUI 的 `BitmapImage` 解 WebP 依赖系统编解码器，较旧的 Windows 10 没有，插画会静默变成空白。
- **把转换脚本放进仓库**：图源改了可以一键重新生成。否决：它依赖 picosvg 和 skia-pathops，仓库里没有这两个依赖；`test-windows-settings-art.py` 能发现图源换了映射或文件不见了，重新生成的办法写在头文件注释里。

## Consequences

- **收益**：Windows 设置的页首和侧栏图标与 macOS 一致，文案只有一份来源（共享 UI），漂移会被 `test-windows-settings-art.py` 和 `settings_navigation.cpp` 拦下。页内标签框架已经把当前标签交给 `show_page_intro_tab`。
- **代价**：`msime-client-settings.exe` 多约 0.9 MB 的 PNG 资源。侧栏图标的路径数据是转换产物，共享 UI 改了某张 SVG 的画面（文件名不变）时，检查脚本看不出来，要人工按头文件注释重新生成。PNG 是否由当前 WebP 逐像素转来，检查只核对尺寸。
- **重访信号**：`settings-mascot-intro.tsx` 增删页面或标签；共享 UI 换图标；Windows App SDK 升级后导航项模板不再用 Viewbox 装图标，或 `PathIcon` 不再按几何的 Transform 绘制。
- 「键盘与手写」「剪贴板」的说明已随标签切换；「语音输入」要等它在 Windows 上分出标签才完整，在那之前固定显示第一个标签的那句。

## Verification

- 本机没有 WinUI 3 / C++/WinRT 工具链，`main.cpp` 和 `Settings.rc` 只能由 PR 的 `CI (platforms)`「Windows native settings」以 `/W4 /WX` 编译链接，并运行 `tests/tools/settings_ui.ps1`：每页确认引导说明是一句话、账号页逐字等于 macOS 的文案、「关于」页的 `settings-page-about` 是产品名；浅深色、800／1000 宽的整页截图用来审看插画、侧栏图标的大小与留白。
- `tests/ui/settings_navigation.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only` 做语法检查，并在 macOS 上用 clang++ 编译运行通过；两个新头文件单独过 `-Wall -Wextra -Wshadow -Werror`。
- `scripts/test-windows-settings-art.py`、`test-windows-settings-ui-pages.py`、`test-windows-editions.py` 通过。
- #9387 合入后的基线在主题页文案对齐断言失败；仅同步 Windows 文案后仍因共享条目改成多行而失败，提取模式兼容空白和尾随逗号后，三个脚本专项均通过。此次仅同步两句说明与提取格式，远端系统卷在构建前余量为 53 GiB，独立检出的 `12ed1eb84a` 通过完整 `bash scripts/verify-local.sh --quick`（退出码 0）：仓库契约、164 项 Windows 可移植原生测试、Rust workspace 检查（未构建设置应用资源，排除 `msime-desktop`）与共享 Apple 桥接编译通过。远端没有 Node，笔记校验在本机单独运行；未安装 MinGW，Windows 原生编译由 PR CI 验证，25 项通过、6 项按条件跳过。不据编译与脚本专项声称 Windows 产品实测。
