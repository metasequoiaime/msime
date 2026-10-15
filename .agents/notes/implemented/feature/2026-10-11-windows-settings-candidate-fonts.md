# Agent Note: Windows 原生设置窗口自己画候选字体各行

Status: implemented

## Problem

原生设置窗口「主题」页「外观」组的「候选字体」只有一行「更改」，用 `settings:appearance` 拉起共享 Tauri 应用的「候选窗口」页。整项工作的目标是任何设置都在原生窗口里改。共享页「字体与大小」组在 Windows 上画四行：「候选字体」预设（`CandidateFontPresetRow`，默认、宋体、黑体、楷体、圆体，圆体在 Windows 上禁用）、「主字体」、「英文字体」（`CandidateFontControls`，带系统字体下拉的输入框）和「系统字体列表」（读取状态和「刷新字体列表」）。预设和主字体同时写 `candidate_font_family` 和有序的 `candidate_fallback_fonts`：Windows 候选窗不读主字体，汉字只从回退列表取。字体列表在 Tauri 里是 `list_font_families`，直接调 host-api 的 `system_fonts::list`，C ABI 只有 `msime_client_resolve_font_families`，没有列出字体的入口。

## Decision

- host-api 新增 `msime_client_list_font_families`，返回 `system_fonts::list` 的结果（DirectWrite 目录，GDI 兜底），与 Tauri 读同一份目录；`msime_client.h` 写明错误码和须在工作线程调用。Tauri 本来就调用共享的 `system_fonts::list`，不用改。
- 预设、主字体、当前预设的判定、字体名校验、字体列表规范化和下拉筛选写成只有头文件的 `CandidateFontPresets.h`，逐条照搬 `candidate-font-presets.ts`、`candidate-font-family.ts`、`font-catalog.ts` 和 `font-family-input.tsx` 在 `platform === "windows"` 下的分支，由 `tests/ui/candidate_font_presets.cpp` 核对，并加进 CMake 测试。
- 「主题」页原来那一行的位置换成这四行，文案照搬共享页。预设用分段控件，`Option` 加了 `enabled`，圆体照共享页禁用；选中预设经 `change` 保存并重画。回退列表用 `SetOrderedStrings` 按给定顺序写。
- 「主字体」「英文字体」用 `AutoSuggestBox`：每输入一个字写进文档，350 ms 后保存，失焦时立即保存；不合法的名字不写入，输入框标红，框下显示共享页底部错误提示里的规则「名称不能为空、不能含控制字符或超过 128 个 UTF-8 字节。」（去掉指向页面的「请在候选窗口页修正字体」，提示就在框下），同一句写进输入框的读屏帮助文本，对应共享输入框的 `aria-invalid`。只标红不够：WinUI 的输入框获得焦点时用自己的焦点边框，打字时看不到红框。主字体输入时只在原处重画「候选字体」那一行，不重建整页，输入框不会丢焦点。
- 字体列表与 `useFontCatalog` 一样，第一次点开字体输入框时才读，「刷新字体列表」重读；在工作线程上读取和解析，按代号丢弃过期的结果，回到 UI 线程后在原处更新状态行（读屏播报的区域）、按钮和正打开的下拉。列表按用户区域设置排序（`CompareStringEx`，对应 `localeCompare`），下拉筛选按用户区域设置转小写（`LCMapStringEx`，对应 `toLocaleLowerCase`），转小写的函数由窗口传给头文件里的 `font_menu`。
- 删掉 `shell_links::appearance`，`all` 从 7 项减为 6 项，导航测试改为核对 `skin`；README 同步。

## Alternatives considered

- **在 C++ 里直接用 DirectWrite 枚举字体** — 不经 Rust；但名字的语言选择、长度和控制字符过滤、16384 项上限都在 `windows_catalog.rs` 里，再写一份就会和共享应用列出的字体不一致。
- **把四行移到原生「候选窗口」页，和共享页放在同一页** — 位置与共享页一致；但原生窗口的「候选窗字号」和原来的「候选字体」都在「主题」页，这次只换掉跳转，不重新安排页面。
- **字体输入框失焦后重建整页来更新预设行** — 实现简单；但点下拉里的字体时可能先失焦，重建会吞掉这次点击。

## Consequences

[Windows 界面参考 macOS 改版](2026-10-15-windows-ui-macos-layout.md)接管页面位置与预览：四行字体设置归入「候选窗口 › 字体大小」，实时预览使用保存的字体。下列原始取舍中的「暂留主题页」与「预览不按字体绘制」由该改版取代；字体目录、校验和回退列表的契约仍由本篇记录。

- 原生窗口不再为候选字体拉起共享应用。
- 预设表和规则写死在 `CandidateFontPresets.h`，`candidate-font-presets.ts` 改了要同步改这里。
- 下拉筛选和浏览器一样按区域设置转小写，个别语言的特殊大小写规则可能与 WebView2 略有出入。
- 原生预览仍不按候选字体绘制。

## Verification

- `cargo test --locked -p msime-host-api --lib font_catalog` 通过，`cargo build --release --locked -p msime-host-api` 通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- `tests/ui/candidate_font_presets.cpp` 和 `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口：输入框、下拉、状态行和预设行的实际交互都未在运行中的窗口里检查。
