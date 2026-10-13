# Agent Note: macOS 屏幕键盘解释发送失败并给出恢复步骤

Status: implemented

## Problem

macOS 屏幕键盘的原生发送层把权限不足、没有外部输入目标和发送前焦点变化都返回为布尔失败，Tauri 再统一转换为 `unavailable`。共享界面只提示“确认输入位置”，用户不能判断应该选择编辑器还是打开辅助功能权限。从设置打开时，窗口也没有解释先选择输入位置的操作顺序。

## Decision

原生发送接口返回有限的结果枚举，Rust 和 Tauri 保留 `permission_denied`、`no_input_target`、`input_target_changed` 与 `unavailable` 错误码，前端仅按这些受控字段提供中文恢复指引。macOS 键盘首次显示就提示先点目标应用的输入框。输入目标捕获失败也报告 `no_input_target`。任何发送失败仍停止连发并丢弃排队按键，不自动重试，也不弹权限请求或改变焦点。

## Alternatives considered

- **只修改通用失败文案**：改动小且不需要扩展跨语言接口，但权限不足时让用户反复选择输入框仍然无效，不能解决无法判断下一步的问题。
- **失败后自动激活之前的应用或请求权限**：操作更少，但先前目标可能已经改变，重试可能把快捷键交给错误位置，权限弹窗也会打断当前输入；恢复必须由用户明确选择输入框或授权。

## Consequences

[手写设置入口修复](2026-10-13-macos-handwriting-settings-entry.md)一并修正共享屏幕键盘标题栏的按钮宽度和字号，避免本篇新增的长恢复指引挤压布局按钮；发送错误与恢复契约不变。

用户能区分权限与焦点问题，界面不输出原生错误内容、进程标识或输入文字。代价是原生、Rust 和前端需要共同维护有限错误码；陌生错误仍回退到通用提示。权限变更由用户在系统设置完成，此改动不确认本次截图的底层原因。

## Verification

- 原生 `shared-keyboard-host-test` 验证发送成功及 8 种失败分支，失败时不投递事件，并区分权限丢失和焦点变化。
- `cargo test -p msime-host-macos --lib`：21 项通过；对应 crate 的 `cargo clippy --all-targets -- -D warnings` 通过。
- `vitest run tests/input/keyboard-repeat.test.tsx`：9 项通过，覆盖错误码恢复指引、停止长按连发及新的手动点击恢复，不展示原始错误详情。
- 前端类型检查和生产构建通过，改动文件的 lint 与格式检查通过。
- 本地 quick、macOS 设置宿主编译和笔记门禁通过；quick 同时完成 macOS 原生宿主、Linux 桌面宿主和 Windows 交叉编译。
- 未安装到当前正在使用的 macOS 应用；没有请求辅助功能授权，也没有向用户的编辑器注入按键。实际窗口验收仍需补齐，不以单测代替装机验证。
