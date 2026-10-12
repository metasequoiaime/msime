# Agent Note: Windows 原生设置窗口在「输入」页选辅助码插件和自定义码表

Status: implemented

## Problem

原生设置窗口「输入」页的辅助码分组有开关、方案和候选窗口显示，方案下拉框只列七个内置方案；自定义码表选不了，辅助码表插件要点「辅助码插件」一行拉起共享 Tauri 应用的 `settings:input` 页去选。整项工作的目标是任何设置都在原生窗口里改。共享的 `HelpcodeSettingsGroup`（`packages/ui/src/settings/pages/helpcode-page.tsx`）在同一个下拉框里依次列出内置方案、资源目录 `helpcodes/custom` 下的自定义码表（`custom/<文件名>`）和已安装的辅助码表插件（值 `pack:<id>`）：选插件只写 `plugins.helpcode_pack_<族>`，选内置或自定义方案时清空它；已选的插件找不到时显示「id（插件，未找到）」，已选的自定义码表读不到时也留在列表里。

## Decision

- `helpcode_rows` 按共享组件重写方案下拉框：选项、标签（「中文名 (英文名)」、「插件名（插件）」）和选择规则逐条照搬；开关、方案和显示的回落值改为 client-core 的 `default_quanpin_helpcode` / `default_shuangpin_helpcode`（只在拿不到出厂偏好时用到）。
- 自定义码表用 `msime_client_helpcode_schemas` 读宿主选项（`runtime-options.json`）的 `resources`，与 Tauri 的 `list_helpcode_schemas` 相同；插件用 `msime_client_plugins` 的 `catalog`（`state_root` 是数据目录，`sound_packs` 传 null），只取 `kind` 为 `helpcode` 的包，与 `useHelpcodePacks` 相同。没有新增 Rust 代码。
- 两份来源都经 `run_host` 在工作线程读，回到 UI 线程后内容有变化才重画「输入」页。自定义码表只读一次（共享页在挂载时读一次）；插件在每次进入「输入」页时标为过期并重读，同时只有一次读取，读取期间再次进入的那一次在读完后补读。
- 复查后再对齐共享组件在 Windows 上的样子：本版本提供全拼或双拼之一（`editionUsesHelpcode`）就两族都列出；「在候选窗口中显示…辅助码」不随开关变灰；去掉开关行上那段 Shift 说明，它只在 `helpcode_shift_entry` 的移动宿主上显示，Windows 为 false；`resources` 与 Tauri 一样只接受绝对路径。
- 删掉「辅助码插件」行和不再使用的 `shell_links::input`，`all` 从 9 项减为 8 项，`tests/ui/settings_navigation.cpp` 去掉对应断言；`input` 路由仍打开原生「输入」页。README 同步。

## Alternatives considered

- **在 host-api 新增一个只列辅助码表插件的 C 函数** — 回答更小；但 `msime_client_plugins` 的 `catalog` 已经是 Tauri 和 HarmonyOS 用的同一份扫描，多一个导出只是换个过滤位置。
- **每次绘制「输入」页都重读插件目录** — 最新；但每次改一个开关都会重画页面，会不停扫目录、重画两遍，共享页也只在进入页面时读。

## Consequences

- 原生窗口不再为辅助码拉起共享应用；插件的导入和删除仍在「插件」页（目前还是共享应用的页面）。
- 在别处导入或删除插件后，要重新进入「输入」页才看得到，与共享页相同；资源目录里新增的自定义码表要重开窗口。
- 选项文字和规则写死在 `main.cpp`，`helpcode-page.tsx` 改了要同步改这里，以及 Android 的 `HelpcodeChoicePolicy`（见 [移动端导入单个辅助码表](2026-10-12-mobile-helpcode-table-import.md)），共有两份照搬。

## Verification

- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口：下拉框的实际内容、插件读取和写入偏好的效果都未在运行中的窗口里检查。
