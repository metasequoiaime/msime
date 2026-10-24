# Agent Note: macOS 语音按住快捷键的 Option 侧别回退

Status: implemented

## Problem

`MSIMEVoiceHoldShortcut` 用设备相关的 Option 侧别位判定右 Option。`NSEvent` 只带通用 Option 位时，即使键码是 61，按住录音也不会启动；现有测试只构造带侧别位的事件，没有覆盖这个输入。

独立 AppKit 文本框中的硬件测试可以观察到完整侧别位，但输入法收到的事件不同。macOS 27.0.1 的 0.54.0 诊断构建中，真实右 Option 事件的键码是 61，`NSEvent.modifierFlags` 与对应 `CGEvent` 的 flags 都只有 `0x80000`，同一时刻 HID 修饰键状态带有右侧位 `0x40`。语音总开关与右 Option 开关均启用、客户端非空，原状态机仍返回 `Action::None`；按下事件还会重复送达。该实机失败路径与缺位回归一致。

## Decision

宿主在修饰键事件边界读取实体修饰键快照，交给原有语音快捷键状态机。事件带通用 Option 位且没有左右 Option 侧别位时，按键码只恢复本次变化的那一侧，另一侧沿用状态机已经观察到的状态。这样处理较早的左侧事件时，即使实体右侧已经按下，也不会提前占用尚未送达的右侧按下沿。事件自带侧别位时保持原有语义，其他修饰键不做扩展。

Caps Lock 和语音共用原生修饰键查询边界；沿用 Caps Lock 测试替换系统查询的方式，合成测试不依赖执行机器的真实按键状态。

## Alternatives considered

- 仅凭键码 61 加通用 Option 位启动：改动少，但左右 Option 同时按住时，右侧松开后通用位仍在，无法区分松开与重复事件。
- 直接用当前实体状态取代所有事件标记：状态来源统一，但排队事件可能晚于实体松手；会改变已有正常原生事件的语义，因此只对缺位输入回退。
- 安装全局键盘监听：能绕过部分输入上下文的问题，但引入另一条快捷键路径及权限要求。本修改保留现有 IMK 路径，无法补救完全未送达的事件。

## Testing

- 缺位事件的右 Option 按下启动，松开结束，重复事件不二次切换。
- 单独左 Option 不启动，左右重叠时分别松开保持正确语义。
- 事件已有侧别位时不被较晚的系统状态覆盖。
- 排队的左侧事件不会提前占用右侧按下沿。
- 缺位输入与排队事件都先以新增断言复现失败，再验证修复后通过。
- macOS 27.0.1 arm64、Command Line Tools，使用已安装的 macOS 26.5 SDK：原生宿主及全部原生测试目标构建通过。默认 27 SDK 在本机缺少 SwiftUI 宏插件。
- `voice-hold-shortcut`、`live-voice-controller`、`live-voice-socket-route`、`http-voice-controller`、`doubao-voice-controller`、`macos-diagnostic-log` 六项测试通过。控制器回归替换原生修饰键查询，验证合成采集器真实进入录音并在右侧松开时停止。
- 同版本 0.54.0 本地回移构建完成实机验收：右 Option 按下触发录音，松开停止，连续两次录音的开始与停止日志均完整，用户确认语音识别文字正常写入文本框。首次使用完成麦克风授权后可正常重试，验收后关闭临时诊断日志。
- `shortcut` 在边框颜色断言失败；未修改的基线 `d9630c47869f47c0a1877450b20c06d14c4b8726` 同样在 `chrome.strokeColor` 与 `SkinColor(picked.border)` 比较处失败，不修改失败基线。
- 已运行仓库 `verify-local.sh --quick`：Rust workspace 检查、macOS 构建及共享 Apple 桥接构建通过；Android 更新任务检查因无 Java Runtime 失败，iOS 豆包测试因本机缺 XCTest 失败，门禁整体未通过。

## Consequences

既有带侧别位事件保持原语义；缺位输入可以借助实体状态启动与结束按住录音。宿主在语音启用时为每个修饰键事件增加一次状态查询，不增加全局事件监听或新的授权请求。

回退读取的是处理事件时的实体快照，无法还原已经结束的短按，也无法补救完全没有送达的事件。IMK 内部事件已确认存在缺位失败路径；同版本本地回移构建已完成右 Option 按住录音、松开停止及识别文字写入文本框的实机验收。

## Note audit

- [macOS 语音设置精简](../../implemented/simplification/2026-10-16-macos-voice-settings-prune.md)：部分重叠；保留既有按住录音选择和空格锁定语义，不改默认值与界面。
- [Caps Lock 轻点切换](../../implemented/feature/2026-10-11-caps-lock-language-switch.md)：部分重叠；复用原生实体状态查询边界，原有行为由其测试覆盖。
- 检索活跃笔记中的 `VoiceHoldShortcut`、按住录音、右 Option，未发现同一回退决定；其他平台的语音页面与本修改无关。
