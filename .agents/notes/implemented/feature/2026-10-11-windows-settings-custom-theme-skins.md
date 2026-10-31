# Agent Note: Windows 原生设置窗口自己画自定义主题与外部皮肤

Status: implemented

## Problem

原生设置窗口「主题」页已经有颜色模式、内置主题卡片和各界面的单独明暗，但「自定义主题」一行用 `settings:skin` 拉起共享 Tauri 应用：候选颜色的七个取色器、外部皮肤包卡片、「外部皮肤」目录一行都只在共享页（`packages/ui/src/settings/pages/skin-page.tsx`）里。整项工作的目标是任何设置都在原生窗口里改。共享页的选择规则不简单：皮肤包按 base 放进浅色或深色槽位（`applyCandidateSkin`、`removeCandidateSkin`），自定义主题用着目录里的包时选中的是包卡片而不是「自定义」卡片，选一个候选颜色会把当时的主题留作底并清掉两个槽位（`updateCandidateColor`）。C ABI 里扫描、解析主题和读图片的函数都已经有了（`msime_client_skin_catalog`、`msime_client_resolve_theme`、`msime_client_skin_resource`）。

## Decision

- 「主题」页照共享页在 Windows 上的样子补齐：页首说明；内置卡片后面每款包一张卡片（`ExternalSkinCard`：明暗、说明、槽位、id · 版本 · 作者、开关、`system` 底的包才有的预览明暗切换，横排和竖排两个缩小的候选窗口，带圆角、最小宽度、背景图和装饰图）和两种明暗各用哪款皮肤的说明；「更多皮肤」组的「外部皮肤」一行（打开目录、刷新皮肤、扫描状态、目录、出错说明和被忽略的目录）；「自定义主题」组的七个取色器（WinUI `ColorPicker` 放在色块的浮出层里，旁边是「跟随主题」）。「自定义主题」的 shell 行去掉；「社区皮肤」一行留着打开共享应用，社区皮肤库和发布是后面的一项。
- 规则和文案逐条搬进 `platforms/windows/settings/ThemeSkinModel.h`（不依赖 Windows 头文件），由 `tests/ui/theme_skin_model.cpp` 核对，按 C++17 和 C++20 都编译。
- 包卡片的配色不在 C++ 里重算 `customCandidatePalette`，而是在扫描时为每款包、每种明暗调一次 `msime_client_resolve_theme`（自定义主题只由这款包组成，底是包的 base，排列方式取包声明的第一种），与 Server 画真实候选窗口用的是同一份解析。「自定义」卡片按共享页只画不带皮肤包的自定义主题，请求里去掉两个槽位和皮肤目录，缓存与顶部预览分开。
- 扫描、解析配色和读图片都在工作线程上；图片字节按手写的小解析器从回答里取出，再在 UI 线程上解码（SVG 用 `SvgImageSource`）。第一次进入页面和「刷新皮肤」总会更新卡片并重读图片，再次进入页面和窗口重新获得焦点时自动重扫，回答没变就不重画，也不显示「正在读取」，免得窗口一获得焦点整页重画、吞掉激活窗口的那次点击。
- 取色器拖动时直接改文档、刷新顶部预览、停下后保存（与滑块相同）；浮出层开着时不重画页面，关上时重画一次。只有颜色真的变了才写，取色器报一次没变的颜色不会把主题切成自定义。
- 社区安装成功或账号同步确实下载、删除了本地皮肤时，经 `skin_catalog_changed` 通知目录失效。这一步在安装回调的图库代次检查之前完成：用户在下载途中切回本地页，已销毁的图库不能吞掉磁盘变化通知。当前本地皮肤页或正在使用皮肤的候选预览立即强制扫描；其他页面留待进入时扫描。在途扫描复用现有的手动重扫队列，旧扫描即使返回相同签名，也会再读一次。安装失败、覆盖确认和无目录变化的同步不触发该通知（#10005）。
- 「打开目录」照 Tauri 的 `open_skin_directory`：在工作线程上建出数据目录下的 `skins`，再在 UI 线程上用 `ShellExecuteW` 在资源管理器里打开。

## Alternatives considered

- **在 C++ 里照 `customCandidatePalette` 合成包卡片的配色** — 不用每次扫描多调几次 C ABI；但 TypeScript 和 Rust 已经各有一份由 `custom-theme-parity.json` 钉住的实现，再写第三份容易走样。调 `msime_client_resolve_theme` 只多读几次包清单，而且在工作线程上。
- **包卡片也用 `msime_client_skin_package` 按需读取** — 能少读没显示的包；但卡片一次全画出来，扫描时一并读完更简单，扫描本来就要读每个清单。
- **把包卡片塞进内置卡片的定宽网格** — 与共享页的轮播更像；但包卡片要放说明、槽位、开关和两个预览，定宽定高的格子放不下，改成网格下面一列整宽的卡片。
- **在原生窗口里也画悬浮工具栏预览** — 包的工具栏样式是 CSS（`toolbar_stylesheet`），原生窗口没有 CSS 引擎；内置主题卡片也不画工具栏，所以不画，`msime_client_skin_toolbar_stylesheet` 这一项没有用上。

## Consequences

- 原生窗口不再为自定义主题、候选颜色和外部皮肤拉起共享应用；`shell_links::skin` 只剩「社区皮肤」一行在用。README 和 `SettingsNavigation.h` 的说明同步。
- 共享页的选择规则或文案改了，`ThemeSkinModel.h` 要跟着改；头文件开头列了它对应的 TypeScript 文件。
- 包卡片的预览是缩小的示意：三个候选、拼音行、按 0.7 缩放的圆角和装饰带，装饰图放在卡片上方的带子里并按对齐方式摆放，没有按共享页 CSS 的精确几何（装饰图压在卡片上沿）去画。
- 没有工具栏预览，所以共享卡片上关于工具栏样式的几条提示不会出现；「当前浏览器无法隐藏皮肤的选中条」也不需要，原生预览直接按配色的 `show_selected_bar` 画。
- 顶部预览在包被选用时仍在 UI 线程上调 `msime_client_resolve_theme`（会读包清单），这是这项之前就有的做法，取色器拖动时每一步也走这条路。

## Verification

- #10005 的补充反馈：从当前源码提取 `skin_catalog_changed`、`scan_skins`、`finish_skin_scan`，以 Clang C++20 和 `-Wall -Wextra -Werror` 编译临时合成调度实验并运行通过。覆盖旧扫描返回相同签名后仍追加扫描、重复通知合并、隐藏页面延后扫描、读取失败保留旧列表及候选预览刷新。这是对实际方法的一次时序实验，未冒充真实 WinUI 窗口验收；原始社区列表慢响应、部分失败的反馈尚未复现。
- `tests/ui/theme_skin_model.cpp` 和 `tests/ui/settings_navigation.cpp` 用 `cl /W4 /WX`（C++17 和 C++20）单独编译并运行通过；设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- 没有改 Rust，没有跑 cargo。
- 没有启动设置窗口：卡片的实际样子、图片解码、取色器浮出层、开关写入后的重画、窗口激活时的重扫和打开资源管理器都没有在运行中的窗口里检查过；`-Wall -Wextra -Werror` 的 GCC/Clang 编译也没有在本机跑。

canvas 包的静态卡片和社区预览现在共用离屏绘制入口，复用候选窗的九宫格、挂件分层、内容／外留白及行距计算，见[canvas 设置预览](../bug-fix/2026-10-20-windows-canvas-settings-preview.md)。传统背景与装饰带的原路径保留。

主题页的版式后来由[主题页整宽卡片轮播与悬浮工具栏页对齐 macOS](2026-10-20-windows-settings-theme-toolbar-pages.md)接管：内置主题和皮肤包进同一个整宽轮播，卡片画横排、竖排候选窗口和按 `toolbar_palette` 原生画的工具栏，上面「把包卡片塞进定宽网格」「不画工具栏预览」两条备选的结论不再成立；主题页也不再有顶部预览，取色器拖动时重画的是轮播里「自定义」卡片的样例（按不带皮肤包的自定义主题解析，不读包清单），上面关于「刷新顶部预览」的两处随之作废；包卡片的配色解析、槽位规则、扫描和图片读取照旧。
