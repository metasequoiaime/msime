# Agent Note: Windows 原生设置窗口自己画「打字统计」页

Status: implemented

## Problem

原生设置窗口的「打字统计」是 Shell 页，只有一行「打开」，用 `settings:typing-statistics` 拉起共享 Tauri 应用。整项工作的目标是任何设置都在原生窗口里改。共享页（`packages/ui/src/settings/typing-statistics.tsx`）在 Windows 上是桌面版：概览六格、六个标签下的柱形图、分时柱、速度折线、日历热力图、按键热力图、两个环形图、方案排行、按日明细和候选命中位置，下面是「记录打字统计」开关、「自动清理」保留期、刷新、打开数据目录和清空。页面上的数字全是 TypeScript 从原始统计现算的（`typing-statistics-model.ts`、`typing-statistics-helpers.ts`、`typing-speed.ts`、`typing-breakdown.ts`、`typing-chart.ts`、`keyboard-heatmap.ts`），C ABI 的 `msime_client_typing_statistics` 能读写统计，却回答不了「统计文件写过没有、最后什么时候写的」，而「统计没有数据」那段说明靠的就是这两项（Tauri 外壳在自己的命令里看文件修改时间）。

## Decision

- 「打字统计」页改成 Native，照共享页的桌面分支（`mobile` 为假、`platform` 为 windows、没有 HarmonyOS 的 `look`）画全部内容，文案、默认标签（「按键」）、选中一天后其他标签只算那一天、桌面上切换标签保留选中、清空前确认、同一时间只有一个请求、5 秒轮询和窗口激活时重读（窗口最小化、请求在途 15 秒内、离上次不到 1 秒时跳过）、读到的与上次相同就不重画，都照共享组件。每次进入页面都像组件重新挂载一样从头读。
- 派生数字照搬成 C++ 头文件 `platforms/windows/settings/TypingStatisticsModel.h`，不依赖 Windows 头文件，由 `tests/ui/typing_statistics_model.cpp` 核对；数字格式照 `toLocaleString("zh-CN")`、`toFixed`（按 double 的精确十进制值、正好一半取较大者）和 `Math.round`。只给手机外观的 `typing-summary.ts` 不移植，Windows 不画它。
- 文件写入状态放进 client-core：`TypingStatisticsStore::status` 返回 `TypingStatisticsStatus { statistics, availability, lastWrittenMs }`，Tauri 的 `typing_statistics_status` 改为调用它；host-api 的请求加顶层 `status: true`，`load`、`set_enabled`、`set_retention`、`reset` 时回答这个状态，用于别的操作时整个请求被拒。不带它时回答与以前一样。
- 图表用 WinUI 形状画：柱和热力图格子是可点的按钮（带提示、读屏名和选中状态），折线、参照虚线和圆点画在随尺寸重画的 `Canvas` 上，环形图用 `PathGeometry` 的弧段按共享页的 SVG 几何画。所有读写都在工作线程上，JSON 也在工作线程上读。
- 偏好没读到时，打字统计页的开关、保留期下拉框和标签照常可用：`toggle_control`、`select_control`、`segmented_control` 加了 `document` 参数，这几处不写偏好。

## Alternatives considered

- **把派生计算移进 client-core，导出给所有宿主共用** — 这样各平台只有一份实现，最强的理由是将来改一条规则不会漏改；而且 client-core 已经为手机外观算了一份 `summary`。没有这样做，因为共享 React 页今天就是这些数字的实现者，要让它改调 Rust，得同时改 Tauri 命令、共享页的数据流和它现有的 vitest，范围远超这一页；只给原生窗口用 Rust 版而 React 继续用 TypeScript 版，仓库里就会有三份（TS、Rust、手机 summary 的 Rust）而不是两份。照前几项（候选字体、翻译服务）的做法，在头文件里逐条移植并配主机单测，日后若决定统一到 Rust，可以照这个测试换实现。
- **在 C++ 里自己看 `typing-statistics.json` 的修改时间** — 不用改 C ABI；但 Tauri 那边还有一份同样的逻辑（含拒绝符号链接的父目录），两边各写一份容易走样。选了把 Tauri 的逻辑搬进 client-core，两边调同一个函数。
- **保留 Shell 页，只在原生窗口画概览** — 改动小；但设置项（记录开关、保留期、清空）仍要拉起共享应用，不满足这项工作的目标。

## Consequences

- 原生窗口不再为「打字统计」拉起共享应用；`SettingsNavigation.h` 里 `stats` 是 Native，导航测试和 README 同步。
- 共享页的规则改了，`TypingStatisticsModel.h` 要跟着改；头文件开头列了它对应的 TypeScript 文件。
- 统计文件很大（永久保留多年）时，每次重画都在 UI 线程上重新算一遍派生数字；读文件和解析 JSON 在工作线程上，只有数据变了才重画。
- 日历热力图有 371 个按钮，切到「趋势」标签时整页重建可能比其他标签慢，没有量过。
- 按键次数来自 TSF 的 Aux 探测与批量记录，发送端必须先设置确认读取模式再写请求；该路径的故障与回归覆盖见 [Windows Aux 确认读取](../bug-fix/2026-10-14-windows-aux-acknowledgement.md)。
- 窗口的布局与共享页不同：每节是一张带标题的卡片，「记录打字统计」和「自动清理」是设置卡片行，按钮和隐私说明在它们下面的卡片里；环形图在卡片宽 520 以上时与图例并排。

## Verification

- `cargo test --locked -p msime-client-core --lib status_reports_whether` 和 `cargo test --locked -p msime-host-api --lib typing_statistics_boundary` 通过（新增的状态回答、拒绝记录类操作带 status、不带 status 时回答不变）；`cargo build --release --locked -p msime-host-api` 和 `cargo check --locked -p msime-desktop` 通过。
- `tests/ui/typing_statistics_model.cpp` 和 `tests/ui/settings_navigation.cpp` 用 `cl /W4 /WX`（C++17 和 C++20）单独编译并运行通过；设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- 没有启动设置窗口：图表的实际样子、点按选中、轮询重画、确认对话框和打开数据目录都没有在运行中的窗口里检查过；`-Wall -Wextra -Werror` 的 GCC/Clang 编译也没有在本机跑。
