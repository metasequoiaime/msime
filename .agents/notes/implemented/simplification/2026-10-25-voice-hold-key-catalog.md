# Agent Note: 按住录音快捷键共用目录

Status: implemented

## Problem

共享设置页的 `voice-hotkeys.ts` 与 Windows 的 `VoiceSettings.h` 分别维护三个按住录音组合键的顺序、名称和缺键默认值。默认值还需要跟随 `VoiceInputPreferences::default()`，只修改偏好或一端选择器会使旧文档在另一端显示不同的选择。

## Decision

`client-providers::voice::hold_key_catalog` 持有按住录音选择器的公开目录，默认值直接读取 `client-core` 的语音偏好。沿用语音语言目录的静态导出方式生成 `voice-hold-key-catalog.ts` 和 `VoiceHoldKeyCatalog.h` 静态投影，React、WinUI 和 Windows 7 通过既有入口消费。选择器按目录顺序寻找第一个开启项，用户选择时遍历目录生成三个布尔字段的补丁。

TypeScript 投影保留 `VOICE_HOLD_KEYS` 的只读元组类型。macOS、桌面和其他宿主的名称分别投影，缺键取共享默认值，显式关闭保持关闭；多开旧文档在读取时不写回。目录不读取用户文档，只导出公开元数据。

## Existing notes audit

- [语音页精简](../../implemented/simplification/2026-10-16-macos-voice-settings-prune.md)：部分重叠，拥有合并选择器的行为，本片收敛其目录来源。
- [Windows 原生语音页](../../implemented/feature/2026-10-11-windows-settings-voice-page.md)：部分重叠，拥有原生布局与快照，本片更新其快捷键来源。
- [语音偏好模型](../../implemented/simplification/2026-10-16-voice-input-preferences.md)：独立决定，继续持有公开偏好类型和默认对象。
- [语音语言目录](../../implemented/simplification/2026-10-22-voice-language-catalog.md)：独立目录，复用其导出与漂移检查模式。

## Alternatives considered

- 通过 C ABI 动态计算选择和写回补丁：可以让算法也只执行一份 Rust，但选择器目前完全同步；增加异步宿主接口会扩大生命周期与错误处理。采用静态目录，让两端仅保留按目录遍历的投影。
- 在 TypeScript 维护目录后生成 C++：最接近现有共享界面，迁移量较小，但缺键默认值仍需复制 Rust 偏好，来源会继续分散。目录放在现有语音公共层，并直接读取偏好默认值。

## Verification

- 三个键的顺序、平台名称、默认选中值与现有行为一致，公共 TypeScript 元组类型保持兼容。
- 27 种缺键、显式开关组合在共享 UI 和 Windows 中均选择同一项，读取不修改输入；选择任一项或不使用时只生成三个键的补丁。
- 生成器提供 `--check`，契约门禁用合成目录替换 C++ 元数据，运行真实选择与补丁实现；React 回归以同样方式替换元数据，检测消费者恢复手写目录。

## Consequences

React 与两套 Windows 设置不再手写键顺序、名称和缺键默认值。生成物成为源码的一部分，需要和 Rust 目录同步；目录中的平台名称属于设置展示，底层按键路由仍由各宿主拥有。后续若选择器支持任意数量的组合键，需要同时重访公开元组类型和 Windows 7 固定控件映射。
