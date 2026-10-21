# Agent Note: Windows 设置「关于」页分标签对齐 macOS

Status: implemented

## Problem

macOS 桌面设置的「关于」页（共享的 `packages/ui/src/settings/pages/about-page.tsx`）顶上是四个页内标签：版本更新、帮助与反馈、系统信息、许可与隐私，一次只显示一个。Windows 原生设置窗口的「关于」是一整条长页：品牌卡、版本与更新、帮助（三段文字卡片）、问题报告、反馈渠道、许可与隐私从上到下铺开。逐项对比还有这些差距：品牌卡只有 64 DIP 图标和产品名；版本行叫「检查更新」，发现新版本时按钮变「前往下载」，结果只是两行字，没有国内镜像和核对命令；多一行 macOS 没有的「复制下载链接」；没有「系统信息」一组和「复制系统信息」；许可与隐私的顺序和文案不同，缺「许可与版权」和「发送哪些内容」；帮助直接铺在页里；反馈渠道三行没有品牌图标。

## Decision

- 「关于」用页内标签框架（`page_tab_bar` 加 `in_tab`，自动化前缀 `about-tab`），标签 id 和名字取 `about-tabs.ts`。「关于」没有页首引导，标签条是正文第一样东西，去掉它与引导相隔的那段上边距。`feedback` 路由打开「帮助与反馈」标签，`help` 路由再打开帮助视图。
- 「版本更新」：品牌卡照 `AboutHeroSection`（62 DIP、圆角 16、`palette_.subtle` 底的标志块里 48 DIP 的产品图标，「Metasequoia IME」小字，22 DIP 半粗体的产品名，`platform-copy.ts` Windows 一支的说明，112 DIP 的 welcome 插画）。产品名仍按版本取 `MSIME_EDITION_DISPLAY_NAME`，不照 macOS 写死「水杉 IME」：窗口标题和侧栏都用版本名，水杉五笔的关于页不能写成另一个产品。
- 「当前版本」一行的说明是版本号，行尾「检查更新」（检查中「正在检查…」），状态写在版本号下面，已是最新时加 64 DIP 的 updated 插画。发现新版本时下面单列结果块：版本句、未签名红字警告、SHA256 和 `Get-FileHash` 核对命令、「国内镜像下载」和打开官网下载页的「前往下载」，都照共享页。结果块只用 `msime_client_update_check` 已经返回的字段（`release_url`、`installer_name`、`installer_sha256`、`signed`），镜像地址按 `mirrorDownloadUrl` 的规则从发布页 tag 和安装包名拼出来，Rust 侧不加字段。安装包名和摘要照 `fromHostUpdate` 先查形状，不合的当作没有。这些拼法和「系统信息」的文字一起放在 `AboutPageModel.h`，由 `tests/ui/about_page_model.cpp` 核对。
- 「复制下载链接」一行去掉，与 macOS 一致。
- 「系统信息」照 `SystemInfoSection`：应用版本（非完整版带版本名，有发布日期时带上）、系统（Windows 加 `RtlGetVersion` 的版本号）、处理器架构（本程序编译成的架构，写法与 Tauri 外壳填的 `std::env::consts::ARCH` 相同）、输入方案；Windows 报不出的内核、桌面环境、输入法框架和设备型号与共享页一样省略。「复制系统信息」的文本格式与 `systemInfoText` 逐字相同，复制成功后按钮显示「已复制」1.6 秒（复用 `mark_copied`）。
- 「许可与隐私」的顺序和文案照共享页：开源许可协议、许可与版权、第三方组件许可、隐私政策、匿名使用统计（短说明）、发送哪些内容。共享页的「发送哪些内容」是进子页的一行，这个窗口没有子页导航，做成同一位置的折叠区（`add_fold`），展开后是 `UsageReportingDetails` 的四组；搜索落到里面的行时展开。
- 「帮助与反馈」先是「帮助」一行（`›`），点开在原处换成帮助视图，对应 macOS 的「帮助」子页：顶上「关于」回到标签页，然后是「开始输入」「候选词释义」「遇到问题」三张术语卡片和「更多 / 打开官网」。卡片的版式和问法照 macOS，内容写 Windows 的（Win+Space、Shift、数字键、翻页键、Ctrl+Shift+Space 和 Alt+Shift+H，「设置 › 时间和语言 › 语言和区域」）；Win+Space 那行的默认方案按本版本的 `MSIME_EDITION_DEFAULT_SCHEME_W` 写，不写死全拼。「候选词释义」照 `GlossColumnPolicy.h` 写 Windows 的键：macOS 的 Option / Control 加数字在 Windows 上是 Alt / Ctrl 加数字，Tab / Shift+Tab 在高亮候选有释义时换列、没有释义时照常翻页，所以「翻页」一行也说明 Tab 只在没有释义时翻页。帮助视图开着时整页只有它；从别的页回到「关于」时关上；建搜索索引时帮助视图也画一遍，搜到卡片时直接打开。
- 反馈渠道三行照共享页的渠道卡片加 38 DIP 的图标块。图标是共享 UI 的三个 SVG 用 `rsvg-convert` 转成的 72 × 72 PNG，打进 `Settings.rc`（RCDATA 120–123）；GitHub 是单色的，共享页把它遮罩成正文色，这里按明暗各转一份。许可写在 `platforms/windows/settings/assets/feedback/NOTICE.md`（ai-logo 的 MIT 原文、IconPark 的 Apache-2.0 原文），由 `Collect-Notices.ps1` 收进安装包的 `THIRD_PARTY_NOTICES.txt`。

## Alternatives considered

- **在 `update_check.rs` 的回答里直接加镜像地址** — 镜像规则只写一处，所有宿主共用。但共享页本来就是在前端按 `mirrorDownloadUrl` 拼的，Rust 那边没有这条规则；只为 Windows 改 Rust 的回答等于把两份规则变成三份。回答里的 tag 页地址和安装包名已经足够，C++ 照共享页拼，并用单测把格式钉住。
- **帮助继续铺在「帮助与反馈」标签里，不做子视图** — 少一套返回逻辑。但那一标签本来就有报告表单和三张渠道卡片，帮助的段落放在最前会把表单挤到下面，也和 macOS 的入口行加子页对不上；插件页已经有原处换视图的做法，照它做一个返回按钮就够。
- **照 macOS 把产品名写成「水杉 IME」** — 与 macOS 逐字一致。但 Windows 各版本同时安装，窗口标题、侧栏和界面验收都按版本名认这个窗口，写死会让非完整版的关于页报错产品名。
- **GitHub 图标用 PathIcon 跟正文色** — 一份路径随明暗自动变色。但三个图标里两个是多色的，只能用位图；GitHub 一个单独走矢量，要再加一套 SVG 路径转 XAML 的流程，按明暗各转一张 PNG 更简单。

## Consequences

- **收益**：四个标签的内容、顺序和文案与 macOS 一一对应；有更新时用户在设置里就能拿到镜像下载和核对命令；反馈问题时一键复制系统信息；帮助的内容按 Windows 的按键重写，不再只是三段介绍文字。
- **代价与已知上限**：帮助卡片的文字写死在 `main.cpp`，Windows 的默认翻页键、全半角快捷键或系统设置路径变了要同步改；镜像前缀和规则是共享页的复制，`app-resources.ts` 的 `downloadMirrorPrefix` 或 `mirrorDownloadUrl` 改了要改 `AboutPageModel.h`；品牌图标的 PNG 是从 SVG 转来的，共享 UI 换图后要重新转并更新 `NOTICE.md`。「发送哪些内容」是折叠区而不是子页，标题行右边是折叠用的「›」而不是跳转箭头。

## Verification

- `AboutPageModel.h` 由 CMake 的测试目标以 C++17 编译（`platforms/windows/CMakeLists.txt` 的 `CMAKE_CXX_STANDARD 17`），不能用 `starts_with` 这类 C++20 的库函数；最初按 C++20 单独检查漏掉了一处 `starts_with`，已改成比较前缀子串。`x86_64-w64-mingw32-g++ -std=c++17 -Wall -Wextra -Wpedantic -Werror -fsyntax-only platforms/windows/tests/ui/about_page_model.cpp` 通过；同一文件用本机 `clang++ -std=c++17` 编译运行通过（系统信息的各项和复制文本、安装包名和摘要的形状、镜像地址、可信度说明的三种情况）。CI 的 Windows GNU 交叉构建只编译测试目标 `windows-about-page-model`，不运行它（`build-cross.sh` 不执行 Windows 可执行文件，也没有哪个 Windows runner 任务跑它）；断言真正执行是在 Windows 上跑 ctest，或在本机由 `scripts/verify-local.sh` 调 `scripts/test-windows-native-run.py` 用宿主编译器编译运行。
- `main.cpp` 依赖 WinUI 头文件，本机编不了，由 PR 的 Windows CI 编译。`platforms/windows/tests/tools/settings_ui.ps1` 在浅色、深色和 800、1000 宽下逐个切四个标签并截图，打开再关上帮助视图，核对「复制下载链接」已经没有、反馈表单和渠道仍在「帮助与反馈」标签里；不点检查更新和复制按钮。没有在真机上看过画出来的样子。
