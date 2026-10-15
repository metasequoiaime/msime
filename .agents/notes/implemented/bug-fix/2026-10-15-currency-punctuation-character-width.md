# Agent Note: 人民币标点跟随全半角状态

Status: implemented

## Problem

#8055：macOS 中文标点下按 `$` 总是输出 U+FFE5 `￥`，切换半角无效。Engine 的固定标点契约直接返回全角符号，host-api 只会把 ASCII 转成全角；英文模式锁定中文标点时，共享 `EnglishModeOutput.h` 也直接返回全角符号。

## Decision

保留 Engine 的固定标点表，在 host-api 派发完成后、生命周期和脚本看到上屏文本前，只将 `$` 的标点尾巴在半角下改成 U+00A5 `¥`。使用本次动作和 `DispatchFacts::tail_chars` 同时确认来源，不全局替换上屏文本。全角仍输出 `￥`；英文标点仍输出 `$`。共享英文模式输出按相同宽度选字。

## Note audit

检索活跃笔记中的全半角、人民币、`character_width` 和全角转换。标点样式笔记与本篇部分重叠：[引号与方头括号样式](../feature/2026-10-13-punctuation-mark-styles.md) 管样式选择，本篇管人民币宽度。生命周期、脚本改写和中英文空格笔记只约束后处理顺序，不改变其决定。macOS 快捷键、Fcitx5 偏好读取和 Windows 输入模式笔记只约束开关传递，与本次缺失的符号转换无关。

## Alternatives considered

- **固定表直接改成 `¥`，再全局把 `¥` 转成全角。** 能复用逐字符宽度转换，Engine 单测和映射也直观。但会改写候选、自定义短语和插件明确提供的符号，还要改变独立 Engine 的固定契约和已有基准。只处理明确来自标点键的尾巴能保留用户选择。
- **只在 macOS `insertText` 前替换 `￥`。** 改动最小且直达报告平台；但拿不到可靠的标点来源，会误改候选，其他宿主也继续出现同一共享缺陷。

## Verification

`cargo test -p msime-host-api --lib --locked`：633 项通过、2 项忽略；`cargo clippy -p msime-host-api --all-targets --locked -- -D warnings`、Rustfmt 和笔记校验通过。旧实现的空闲人民币用例失败（`￥` 与 `¥` 不同），修复后通过。host-api 回归覆盖字符、标点、带上下文标点三条 C ABI 路径在空闲和组字中的输出，以及全角、半角反复切换；英文标点、Caps Lock、候选原文和其他中文标点保持原有语义。快捷短语含 `￥¥` 时只转换本次按键附加的尾巴。Linux 的 `smart_punctuation_space.cpp` 在 macOS 上直接编译运行通过，覆盖共享英文模式的人民币宽度与英文标点；这不是 Linux 输入法系统入口验收。macOS 原生事件测试覆盖英文模式锁定中文标点时的上屏，Apple 桥接测试用真实 Engine 验证空闲与组字中的切换。`ctest --test-dir target/macos-isolated -R '^(shortcut|text-client)$' --output-on-failure` 两项通过；本地 quick 门禁通过，含 Windows x64 原生链接、Windows GNU 设置组件检查、Android 与 WebAssembly 编译、macOS 原生宿主与 Apple 桥接编译。Linux 容器与 HarmonyOS 真打包因本机对应环境未就绪跳过，没有在已注册的系统输入法与实际编辑器中装机验收。

## Consequences

收益是人民币标点立即跟随已有宽度开关，无需迁移偏好或改变 Engine 固定表。尾巴必须来自本次派发，不能把旧事实用于设置调用。用户明确选择的人民币候选按候选原文输出，因此本次不提供任意文本的通用宽度归一化。Windows TIP 自行输出标点的旧路径不经过这里，本篇不宣称其独立按键路径已覆盖。
