# Agent Note: Windows 原生设置窗口绘制反馈表单

Status: implemented

## Problem

原生设置窗口「帮助与反馈」页的「反馈」一行拉起共享 Tauri 应用的 `settings:feedback` 页。整项工作的目标是任何设置页都不再拉起共享应用。共享的 `FeedbackSettingsPage`（`packages/ui/src/settings/pages/feedback-page.tsx`）在 Windows 上走非鸿蒙的分支：页首说明、「帮助」入口、「提交可复现的问题」（反馈类型、最多 4000 字的描述、会一起附上的信息、「复制报告」「在 GitHub 提交」、诊断日志入口）和「反馈与交流」的 GitHub、QQ、Telegram 三张卡片。报告格式、截断和地址编码在 `use-feedback-report.ts`，诊断信息在 `support-diagnostics.ts`。

## Decision

- 新的 `settings/FeedbackReport.h`（不依赖 Windows 头文件）照搬反馈类型、`schemeTitle`、`supportDiagnostics`（宿主 windows）、报告正文 `### 类型 / ### 描述 / ### 环境`、`URLSearchParams` 的表单编码（UTF-8、空格写 `+`、只留字母数字和 `*-._`、落单代理项换 U+FFFD）和按 UTF-16 码元截到 4000 的正文。`tests/ui/feedback_report.cpp` 的期望值取自浏览器 `URLSearchParams` 的实际输出，并登记进 CMake。
- `build_feedback_page` 按共享页的顺序绘制：说明、帮助内容（本窗口没有子页，沿用上一步的 `build_help_section`）、报告分组、渠道分组。文案逐字照搬；诊断日志一行改用共享页的说明「在维护与诊断页开启日志并打开日志目录」。
- 诊断信息的系统版本来自 `RtlGetVersion`，走共享函数里「宿主报告了 `os_version`」那一支，写成「Windows 10.0.<内部版本号>」；读不到时写「平台：windows」。输入方案只在偏好读到时写，与共享页没有草稿时不写一致。
- 类型、描述和「已复制」标记是窗口的成员，页面重建（保存、刷新、切页）时保留，关窗即丢，与共享页的组件状态相同。「已复制」1.6 秒后复原。
- 「复制报告」「复制群号」与共享页一样只在复制成功后才改成「已复制」；剪贴板被别的程序占着时 `copy_text` 捕获 `hresult_error` 并提示「复制失败，请重试」（文案取自共享面板），不让异常漏出点击回调。复制后 `Clipboard::Flush`，关窗后群号和报告仍在剪贴板里。「关于」页的「复制链接」走同一函数。
- TextBox 的换行是 CR，生成报告前统一换成 LF，与 textarea 的值一致。
- 删掉不再使用的 `shell_links::feedback`，`all` 从 10 项减为 9 项；导航测试把 `feedback` 列入原生页。README 同步。

## Alternatives considered

- **系统版本照 Tauri 在 Windows 上的实际输出写「平台：windows」加 User-Agent** — 本窗口没有浏览器 User-Agent，伪造一个没有意义；共享函数本来就为报告了 `os_version` 的宿主准备了「<系统> <版本>」一支，用它信息更准。
- **把报告和编码放进 host-api 的 Rust 函数** — 只是字符串拼接和百分号编码，没有文件或网络；放在头文件里可以像其他设置辅助头一样单独测试，不必为它加一个 C 接口。
- **用 `select_control` 画类型下拉框** — 它在偏好没读到时禁用，而反馈类型不是偏好，应始终可选，所以直接建 ComboBox。

## Consequences

- 「帮助与反馈」页不再有拉起共享应用的入口。
- 报告格式、反馈类型和渠道信息写死在 `FeedbackReport.h` 和 `main.cpp`，共享页的这些改了要同步改这里。
- 「在 GitHub 提交」与 Tauri 一样经 `ShellExecuteW` 打开地址；描述接近 4000 个汉字时编码后的地址会超过三万字符，浏览器能否接受这么长的地址与 Tauri 应用相同，未在本机验证。

## Verification

- `tests/ui/feedback_report.cpp` 与 `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。没有启动窗口，页面排版、剪贴板和浏览器打开均未实际操作验证。
