# Agent Note: Android 空格复制失败提示显示在屏幕顶部

Status: implemented

## Problem

#11112 要求将复制失败的黑色提示从候选区诊断行移到屏幕顶部中央，工具栏保留图标和高度。初稿的键盘内浮层虽不参与测量，却仍覆盖工具栏，与附件绿色标注不符；原来的手势进度提示只是空格键面文字，并不是这条失败通知。

## Decision

复制失败返回布尔结果，由空格手势入口显示独立的 `ImeOperationNotice` 实例。复用已存在的屏幕顶部定位、旧版自定义 Toast 和 Android 11+ 不取焦点的输入法子窗口，提供独立的无障碍描述、窗口标题及原有四秒超时。常用语提示仍用自身实例、原描述和三秒时长。输入法换编辑器、隐藏、重建或销毁时撤销复制提示，避免旧结果出现在下一次输入中。

手势中的松开/取消标签继续在空格键内显示。复制仍通过编辑器的菜单动作完成，失败时不读取正文自行写剪贴板。

## Related notes audit

[键盘内浮层方案](../../rejected/feature/2026-11-10-android-space-copy-hint-overlay.md)因位置不符合要求转为 rejected，不让“不改变测量”替代“显示到指定位置”。常用语顶部提示的窗口实现复用，不改变其行为。

## Alternatives considered

- 键盘内浮层：实现简单、布局稳定，但覆盖工具栏且不在用户标注的屏幕区域，因此不用。
- 全部系统使用文字 Toast：无需管理子窗口，但 Android 11 起忽略定位，不能保证屏幕顶部中央。

## Consequences

失败通知不再参与候选区测量；代价是必须在输入法生命周期变化时移除子窗口。设备回归同时检查顶部位置、提示出现与消失期间输入法窗口持续存在、按键位置及编辑器焦点不变。旧系统结合固定通知文字和实际像素定位，新系统核对独立窗口的可访问节点；截图只含专用编辑器的合成内容。

`smoke.sh --space-copy` 定向运行真实主输入与完整空格复制专项；共享设备 workflow 仍执行 Rust 设备单测及崩溃专项。API 28/35 的实际结果须完成后记录，不能由普通 PR 冒烟替代。

## Device evidence

运行 `42787159009` 的 API 28 通过 Rust 设备单测及崩溃专项，但新增提示断言停在拒绝复制的编辑器上。[Android 9 的跨进程 `InputConnectionWrapper.performContextMenuAction`](https://android.googlesource.com/platform/frameworks/base/+/android-9.0.0_r1/core/java/com/android/internal/view/InputConnectionWrapper.java) 只把命令发给编辑器并返回 true，不回传编辑器自身的拒绝结果，因此这不能作为提示必现的前提。原有拒绝复制、剪贴板不能被输入法自行改写的断言保留；提示出现、位置、自动消失和稳定性改在 issue 对应的空输入框上验证。空输入框的完整提取快照长度为零，`EditorCopyAll` 会明确拒绝复制，不依赖跨进程布尔回传。首次检查帧与成功提示帧分别保留，便于识别通知未出现还是旧系统像素检测未识别。


API 35 的 `42791187286` 已实际通过顶部位置、键盘边界、焦点与自动消失检查，并取得提示截图；失败发生在随后新增的空框选区检查。Android 15 的 [`View.onInitializeAccessibilityNodeInfoInternal`](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android15-release/core/java/android/view/View.java) 只有 `text.length() > 0` 时才写标准 `textSelection`，空框节点返回 -1 不代表 EditText 光标被移动。合成编辑器通过节点 extras 暴露自身 `getSelectionStart/End()`，空框用例要求真实选区仍为 0:0、文本为空、焦点保留；旧的所有非空选区断言不变。修正后仍需双 API 专项通过，不把已通过的提示子检查冒充整套成功。


最终双 API 专项 `42795596706` 在提交 `b81418ed9b7d0b801674011f9bb843c89c4aff02` 上全部成功：Rust 设备单测、崩溃专项、主输入、完整空格复制专项均通过，空框实际选区为 0:0，顶部通知不改变键盘窗口与按键边界，并按时消失。API 28/35 均保存成功截图。随后同步 develop 的按键字面修复，保留 `--space-copy` 与 `--key-faces` 两个定向入口及各自截图收集。
