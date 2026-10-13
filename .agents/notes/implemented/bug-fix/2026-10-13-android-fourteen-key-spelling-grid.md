# Agent Note: Android 14 键拼音使用独立换行选择区

Status: implemented

## Problem

14 键的拼音选项挤在 14dp 高的读音行右侧，读音最多再占半行。用户需要横向滑动细小文字来消歧，触摸目标过小且可见选项不足。

## Decision

本布局曾构建、通过设备验收并安装到手机，现由 [与九键共用左侧栏](2026-10-13-android-fourteen-key-left-spellings.md) 取代。保留以下当时的取舍，说明为什么不继续使用独立换行区。

Android 在候选词下方提供独立拼音选择区，每项至少 64dp 宽、48dp 高，按可用宽度换行。最多显示两行，更多选项纵向滚动。读音恢复整行宽度，候选词不被覆盖，字母键的布局和高度不变。没有可选拼音或回落其他布局时选择区不占空间。

复用 `WrapRowLayout` 和已有带 generation、索引的拼音选择回调，不修改引擎解码、锁定、退格和九键侧栏。仅调整 Android；iOS、HarmonyOS 暂沿用现有位置。

历史检索：[全拼 14 键](../../implemented/feature/2026-10-10-fourteen-key.md) 直接拥有原来的三端读音行横向选择决定，本笔记仅取代其中 Android 的显示位置和触摸尺寸，两篇互链。数字键盘设置裁剪、九键双拼和其他命中笔记不负责此交互。

## Alternatives considered

- 加高读音行并保留横向选择：可保持原有结构且改动更小，但读音仍分走宽度、选项仍靠横向滑动发现。
- 直接盖在候选行上：可复用注音九键容器且不增加键盘高度，但无法同时看拼音和候选词。
- 全部选项展开且不限高度：每项直接可点，但大量选项和浮动窄键盘会挤占编辑区域，因此限制为两行并允许纵向滚动。

## Consequences

拼音可直接点按、读音完整且候选词持续可见。代价是组字时键盘增加一至两行高度；窄屏更多选项仍需纵向滚动。引擎组码、锁定与退格行为不变。

Android 宿主检查、原生 APK 构建、`verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check` 通过。专用 API 35 AVD 的核心打字、`FourteenKeyDeviceSmoke`、`NineKeyPanelDeviceSmoke` 和 `KeyboardHeightDeviceSmoke` 均通过。

14 键设备套件覆盖候选区分离、64×48dp 触摸目标、约 183dp 窄屏换行、拼音锁定 / 退格撤销、英文与密码回落；九键套件确认共享容器调整未破坏侧栏和展开面板，高度套件确认字母键与底行仍等高。测试偏好只写入专用 AVD，窄屏检查结束后恢复显示尺寸。
