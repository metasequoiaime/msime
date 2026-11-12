# Agent Note: 独立剪贴板面板保留标题栏拖动

Status: implemented

## Problem

独立剪贴板窗口和设置页内嵌面板共用 `DesktopEmojiPanel`。宿主为剪贴板面板装配客户端时，原逻辑同时移除了 `beginWindowDrag` 与 `resizeWindow`，导致独立剪贴板窗口的标题栏也不能拖动。调整窗口大小仍不属于剪贴板面板的能力，但移动窗口是必要的窗口操作。

## Decision

`DesktopEmojiPanel` 在设置页内嵌时继续移除全部窗口几何能力；独立表情窗口保留拖动和缩放；独立剪贴板窗口保留 `beginWindowDrag`，只移除 `resizeWindow`。共享 `EmojiPanel` 的标题栏手势继续负责拖动，宿主不复制手势判定。

独立剪贴板的拖动回归测试通过标题栏的真实指针按下和移动事件验证，设置页内嵌面板则明确验证不会调用宿主拖动接口。

## Related notes

- [桌面符号直接选择子分类并允许调整面板大小](../feature/2026-10-21-desktop-symbol-leaf-categories-and-resize.md)：原先记录了独立剪贴板不透传几何能力；本篇收窄该约定，保留其不可缩放边界。
- [桌面表情面板拆分](../simplification/2026-10-18-desktop-emoji-panel.md)：继续复用 `DesktopEmojiPanel` 的客户端装配与生命周期。

## Alternatives considered

- 给独立剪贴板同时开放缩放：实现简单，但剪贴板面板没有经过尺寸适配验收，且原有最小尺寸契约只针对独立表情窗口，因此保留不可缩放边界。
- 在剪贴板宿主中另写标题栏拖动：可以绕过共享面板，但会复制 `usePanelDrag` 的按钮排除、阈值和失焦清理逻辑，容易产生行为分叉，因此继续复用共享实现。

## Consequences

独立剪贴板窗口可以通过标题栏移动，窗口尺寸保持固定；设置页内嵌面板不会把拖动事件交给宿主窗口。测试覆盖了三种装配模式的缩放与拖动能力边界。真实 Windows 窗口和报告者环境的验收仍需在 Draft PR 中补做。
